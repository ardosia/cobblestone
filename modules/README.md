Cobblestone no longer uses a PHP package workspace under `modules/`.

PHP product code lives in the single root Composer package:

```text
src/
├── Command/
├── Event/
├── Log/
├── Native/
├── Plugin/
├── Server/
├── Session/
├── Task/
├── Tick/
└── World/
```

`Cobblestone\\` maps directly to `src/`. World generation, lighting, and mutation are namespace subdomains under `src/World/`, not separately versioned Composer packages.

The remaining `modules/` tree is the current Rust Cargo workspace:
```text
modules/rust/
├── core/
├── codec/
├── network/
├── session/
├── storage/
└── php-extension/
```

These crate paths remain unchanged during the PHP-layout migration. A later native-workspace cleanup may rename/restructure them, but this document describes current durable repository state.

Repository-level PHP layout does not control Zend export names. The native extension owns its ABI independently and exact exports remain regression-tested.
