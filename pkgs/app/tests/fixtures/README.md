# Project compatibility fixture

`legacy-project.zip` is a fixed version-1 archive from the former web app. It preserves:

- String seeds.
- Camel-case editor properties.
- Snake-case character properties.

The compatibility test loads and resaves this archive without using the current serializer to create it.
