Releasing
===

1. Update the cargo.toml and change the `workspace.package.version`.
2. Change the `dependency.edfsm`* versions to be the same version number as per step 1.
3. Commit the changes and tag with the version using `v` as a prefix e.g. 1.0.0 would be "v1.0.0".
4. Perform the commands below, and in the same order...

```
cargo publish -p edfsm-macros
cargo publish -p edfsm
cargo publish -p edfsm-machine
cargo publish -p edfsm-kv-store
```