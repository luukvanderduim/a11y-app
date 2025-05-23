# a11y-app

Small utility to print properties on an accessible application (or the whole bus tree).

## Example output

```console
luuk@n ~> a11y-app kat
Sought kat, partially matches application: kate
Would you like to add this application? (Y/n)

Sought kat, partially matches application: kate
Would you like to add this application? (Y/n)

Application: kate (:1.49) - Accessible Properties of its root object:
+----------------+-------------------------------------------------------------------------------------------------------------------------------------------------+
| Name:          | kate                                                                                                                                            |
| Description:   | /usr/bin/kate                                                                                                                                   |
| Locale:        | Error: org.freedesktop.DBus.Error.UnknownInterface: Interface org.a11y.atspi.Accessible was not found in object /org/a11y/atspi/accessible/root |
| Accessible ID: | org.kde.kate                                                                                                                                    |
| Child count:   | 1                                                                                                                                               |
| Parent:        | ObjectRef { name: OwnedUniqueName(":1.49"), path: OwnedObjectPath(ObjectPath("/org/a11y/atspi/null")) }                                         |
| Help text:     | --- No value ---                                                                                                                                |
+----------------+-------------------------------------------------------------------------------------------------------------------------------------------------+

Application: kate (:1.48) - Accessible Properties of its root object:
+----------------+-------------------------------------------------------------------------------------------------------------------+
| Name:          | kate                                                                                                              |
| Description:   | --- No value ---                                                                                                  |
| Locale:        | nl_NL.UTF-8                                                                                                       |
| Accessible ID: | --- No value ---                                                                                                  |
| Child count:   | 0                                                                                                                 |
| Parent:        | ObjectRef { name: OwnedUniqueName(":1.3"), path: OwnedObjectPath(ObjectPath("/org/a11y/atspi/accessible/root")) } |
| Help text:     | --- No value ---                                                                                                  |
+----------------+-------------------------------------------------------------------------------------------------------------------+
```

## Building

To build the application, you need to have Rust and Cargo installed. Then, run the following command in the project directory:

```sh
cargo build --release
```

The executable will be located at `target/release/a11y-app`.

## Installation

```sh
cargo install --path .
```

If all is well, `a11y-app` will be in your `.cargo/bin/`
Which you should have in your path.

### Examples

1. **View properties of the AT-SPI registry (default):**

    ```sh
    ./target/release/a11y-app
    ```

    Or explicitly:

    ```sh
    ./target/release/a11y-app org.a11y.atspi.Registry
    ```

2. **View properties of a specific application by its accessibility name (e.g., "gedit"):**

    ```sh
    ./target/release/a11y-app gedit
    ```

    *(Note: The application must be running and have an accessible interface registered.)*

3. **View properties and print the accessibility tree of a specific application by its name:**

    ```sh
    ./target/release/a11y-app "gedit" -p
    ```

## License

MIT
