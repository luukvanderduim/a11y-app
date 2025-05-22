use argh::FromArgs;
use atspi::{
    connection::set_session_accessibility,
    proxy::accessible::{AccessibleProxy, ObjectRefExt},
    zbus::{proxy::CacheProperties, Connection},
    AccessibilityConnection, Role,
};
use display_tree::{AsTree, DisplayTree, Style};
use futures::executor::block_on;
use futures::future::try_join_all;
use std::vec;
use zbus::names::BusName;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type ArgResult<T> = std::result::Result<T, String>;

const REGISTRY_DEST: &str = "org.a11y.atspi.Registry";
const ACCESSIBLE_ROOT: &str = "/org/a11y/atspi/accessible/root";
const ACCESSIBLE_INTERFACE: &str = "org.a11y.atspi.Accessible";

#[derive(Debug, PartialEq, Eq, Clone)]
struct A11yNode {
    role: Role,
    children: Vec<A11yNode>,
}

impl DisplayTree for A11yNode {
    fn fmt(&self, f: &mut std::fmt::Formatter, style: Style) -> std::fmt::Result {
        self.fmt_with(f, style, &mut vec![])
    }
}

impl A11yNode {
    fn fmt_with(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        style: Style,
        prefix: &mut Vec<bool>,
    ) -> std::fmt::Result {
        for (i, is_last_at_i) in prefix.iter().enumerate() {
            // if it is the last portion of the line
            let is_last = i == prefix.len() - 1;
            match (is_last, *is_last_at_i) {
                (true, true) => write!(f, "{}", style.char_set.end_connector)?,
                (true, false) => write!(f, "{}", style.char_set.connector)?,
                // four spaces to emulate `tree`
                (false, true) => write!(f, "    ")?,
                // three spaces and vertical char
                (false, false) => write!(f, "{}   ", style.char_set.vertical)?,
            }
        }

        // two horizontal chars to mimic `tree`
        writeln!(
            f,
            "{}{} {}",
            style.char_set.horizontal, style.char_set.horizontal, self.role
        )?;

        for (i, child) in self.children.iter().enumerate() {
            prefix.push(i == self.children.len() - 1);
            child.fmt_with(f, style, prefix)?;
            prefix.pop();
        }

        Ok(())
    }
}

impl A11yNode {
    async fn from_accessible_proxy_iterative(ap: AccessibleProxy<'_>) -> Result<A11yNode> {
        let connection = ap.inner().connection().clone();
        // Contains the processed `A11yNode`'s.
        let mut nodes: Vec<A11yNode> = Vec::new();

        // Contains the `AccessibleProxy` yet to be processed.
        let mut stack: Vec<AccessibleProxy> = vec![ap];

        // If the stack has an `AccessibleProxy`, we take the last.
        while let Some(ap) = stack.pop() {
            let child_objects = ap.get_children().await?;
            let mut children_proxies = try_join_all(
                child_objects
                    .into_iter()
                    .map(|child| child.into_accessible_proxy(&connection)),
            )
            .await?;

            let roles = try_join_all(children_proxies.iter().map(|child| child.get_role())).await?;
            stack.append(&mut children_proxies);

            let children = roles
                .into_iter()
                .map(|role| A11yNode {
                    role,
                    children: Vec::new(),
                })
                .collect::<Vec<_>>();

            let role = ap.get_role().await?;
            nodes.push(A11yNode { role, children });
        }

        let mut fold_stack: Vec<A11yNode> = Vec::with_capacity(nodes.len());

        while let Some(mut node) = nodes.pop() {
            if node.children.is_empty() {
                fold_stack.push(node);
                continue;
            }

            // If the node has children, we fold in the children from 'fold_stack'.
            // There may be more on 'fold_stack' than the node requires.
            let begin = fold_stack.len().saturating_sub(node.children.len());
            node.children = fold_stack.split_off(begin);
            fold_stack.push(node);
        }

        fold_stack.pop().ok_or("No root node built".into())
    }
}

async fn get_registry_accessible<'a>(conn: &Connection) -> Result<AccessibleProxy<'a>> {
    let registry = AccessibleProxy::builder(conn)
        .destination(REGISTRY_DEST)?
        .path(ACCESSIBLE_ROOT)?
        .interface(ACCESSIBLE_INTERFACE)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;

    Ok(registry)
}

/// Select the bus name to be used
#[derive(FromArgs)]
struct AccessibleBusName {
    /// the bus name or application name to be used
    /// (default: org.a11y.atspi.Registry)
    #[argh(positional, from_str_fn(parse_bus_name))]
    bus_name: zbus::names::BusName<'static>,

    /// whether to print the tree
    #[argh(switch, short = 'p')]
    print_tree: bool,
}

/// Parse the bus name from the command line argument
fn parse_bus_name(name: &str) -> ArgResult<BusName<'static>> {
    // If the name is empty, use the default bus name
    if name.is_empty() {
        match BusName::try_from(REGISTRY_DEST) {
            Ok(name) => return Ok(name.to_owned()),
            Err(e) => return Err(format!("Invalid bus name: {REGISTRY_DEST} ({e})")),
        };
    }

    match BusName::try_from(name) {
        Ok(name) => Ok(name.to_owned()),
        _ => {
            // If the name is not a valid bus-name, try to parse it as an application name
            bus_name_from_app_name(name)
        }
    }
}

/// BusName from application name
fn bus_name_from_app_name(candidate_name: &str) -> ArgResult<BusName<'static>> {
    let a11y = block_on(AccessibilityConnection::new()).map_err(|e| e.to_string())?;
    let conn = a11y.connection();
    let registry_accessible = block_on(get_registry_accessible(conn)).map_err(|e| e.to_string())?;

    let mut apps = block_on(registry_accessible.get_children()).map_err(|e| e.to_string())?;

    // get vec in reverse order - newest apps first
    apps.reverse();

    // turn the vec into vec of AccessibleProxies
    for app in apps {
        let bus_name = app.name.clone();
        let acc_proxy = block_on(app.into_accessible_proxy(conn));
        let acc_proxy = match acc_proxy {
            Ok(acc_proxy) => acc_proxy,
            Err(e) => {
                eprintln!(
                    "warn: {} could not convert to accessible proxy: {}",
                    &bus_name, e
                );
                continue;
            }
        };

        let name = match block_on(acc_proxy.name()) {
            Ok(name) => name,
            Err(e) => {
                eprintln!("warn: {:?} returned an error getting name: {e}", &bus_name);
                continue;
            }
        };

        match (name == candidate_name, name.contains(candidate_name)) {
            (true, _) => return Ok(BusName::from(bus_name)),
            (false, true) => {
                // If the name contains the candidate name, ask the user
                println!("Found application: {name}");
                println!("Do you want to use this application? (Y/n)");
                let mut answer = String::new();
                std::io::stdin()
                    .read_line(&mut answer)
                    .expect("Failed to read line");
                let answer = answer.trim().to_lowercase();
                if answer == "y" || answer == "yes" || answer.is_empty() {
                    return Ok(BusName::from(bus_name));
                } else if answer == "n" || answer == "no" {
                    continue;
                } else {
                    return Err(format!("Invalid answer: {answer}"));
                }
            }
            (false, false) => continue,
        };
    }
    Err(format!("No application found with name: {candidate_name}"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: AccessibleBusName = argh::from_env();
    let bus_name = args.bus_name.clone();
    println!("Using bus name: {bus_name}");

    set_session_accessibility(true).await?;
    let a11y = AccessibilityConnection::new().await?;
    let conn = a11y.connection();
    let root_accessible = get_root_accessible(conn, &args.bus_name).await?;

    println!("Properties of the root accessible object:");
    let empty = "--- No value ---".to_string();

    let name_property = {
        let res = root_accessible.name().await;
        match res {
            Ok(name) if name.is_empty() => empty.clone(),
            Ok(name) => name,
            Err(e) => format!("Error: {e}"),
        }
    };
    let description_property = {
        let res = root_accessible.description().await;
        match res {
            Ok(description) if description.is_empty() => empty.clone(),
            Ok(description) => description,
            Err(e) => format!("Error: {e}"),
        }
    };
    let locale_property = {
        let res = root_accessible.locale().await;
        match res {
            Ok(locale) if locale.is_empty() => empty.clone(),
            Ok(locale) => locale,
            Err(e) => format!("Error: {e}"),
        }
    };
    let accessible_id_property = {
        let res = root_accessible.accessible_id().await;
        match res {
            Ok(accessible_id) if accessible_id.is_empty() => empty.clone(),
            Ok(accessible_id) => accessible_id,
            Err(e) => format!("Error: {e}"),
        }
    };
    let child_count_property = {
        let res = root_accessible.child_count().await;
        match res {
            Ok(child_count) => child_count.to_string(),
            Err(e) => format!("Error: {e}"),
        }
    };
    let parent_property = {
        let res = root_accessible.parent().await;
        match res {
            Ok(parent) => format!("{parent:?}"),
            Err(e) => format!("Error: {e}"),
        }
    };
    let help_text_property = {
        let res = root_accessible.help_text().await;
        match res {
            Ok(help_text) if help_text.is_empty() => empty.clone(),
            Ok(help_text) => help_text,
            Err(e) => format!("Error: {e}"),
        }
    };

    let props_data = [
        ("Name:", name_property),
        ("Description:", description_property),
        ("Locale:", locale_property),
        ("Accessible ID:", accessible_id_property),
        ("Child count:", child_count_property),
        ("Parent:", parent_property),
        ("Help text:", help_text_property),
    ];

    // Determine maximum widths for each column
    let max_label_width = props_data
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(0);
    let max_value_width = props_data
        .iter()
        .map(|(_, value)| value.len())
        .max()
        .unwrap_or(0);

    // Create the horizontal border string
    let label_border_segment = "-".repeat(max_label_width + 2); // +2 for " " padding
    let value_border_segment = "-".repeat(max_value_width + 2); // +2 for " " padding
    let horizontal_border = format!("+{label_border_segment}+{value_border_segment}+");

    // Print the top border
    println!("{horizontal_border}");

    // Print property rows
    for (label, value) in &props_data {
        println!("| {label:<max_label_width$} | {value:<max_value_width$} |");
    }

    // Print the bottom border
    println!("{horizontal_border}");

    if args.print_tree {
        println!("Press 'Enter' to print the tree...");
        let _ = std::io::stdin().read_line(&mut String::new());
        println!("Construct a tree of accessible objects of the bus name\n");

        let tree = A11yNode::from_accessible_proxy_iterative(root_accessible).await?;

        println!("{}", AsTree::new(&tree));
    }

    Ok(())
}

async fn get_root_accessible<'a>(
    conn: &'a Connection,
    bus_name: &BusName<'a>,
) -> Result<AccessibleProxy<'a>> {
    let root_accessible = AccessibleProxy::builder(conn)
        .destination(bus_name)?
        .path(ACCESSIBLE_ROOT)?
        .interface(ACCESSIBLE_INTERFACE)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;

    Ok(root_accessible)
}
