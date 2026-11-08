# Project compatibility fixture

`legacy-project.zip` is a fixed version-1 archive using the former web app's YAML field names and values, including string seeds, camel-case editor properties, and snake-case character properties. The compatibility test loads and resaves it rather than constructing it through the current serializer.
