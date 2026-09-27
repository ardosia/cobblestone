# First-party modules

`modules/` contains first-party gameplay/domain features that sit above the Cobblestone kernel.

Composer maps this directory to the `Cobblestone\\Modules\\` namespace. A class at:

```text
modules/World/Chunk.php
```

uses:

```php
namespace Cobblestone\Modules\World;
```

This is a source-layout boundary only. It is deliberately **not** a runtime module loader, plugin discovery system, or Zend/PHP-extension mechanism.

Native extension functions such as `cobblestone_session_protocol84_accept_login()` are registered by `ext-php-rs` and are independent from Composer/PSR-4 class autoloading. The PHP integration suite contains an exact export-name regression check for that boundary.

Kernel/runtime infrastructure stays under `src/Cobblestone/`. Third-party/user extensions remain plugins. New first-party gameplay areas such as World, Player, Entity, and Inventory should become modules when their implementation begins rather than pre-creating empty frameworks.
