# Themes Example

This is an example of a M31A extension that adds a custom theme.

## How to use

1.  Link this extension:

    ```bash
    m31a extensions link packages/cli/src/commands/extensions/examples/themes-example
    ```

2.  Set the theme in your settings file (`~/.m31a/settings.json`):

    ```json
    {
      "ui": {
        "theme": "shades-of-green (themes-example)"
      }
    }
    ```

    Alternatively, you can set it through the UI by running `m31a` and then
    typing `/theme` and pressing Enter.

3.  **Observe the Changes:**

    After setting the theme, you should see the changes reflected in the M31A
    UI. The background will be a dark green, the primary text a lighter green,
    and various other UI elements will display different shades of green, as
    defined in this extension's `m31a-extension.json` file.
