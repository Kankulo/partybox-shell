# partybox-shell

Envoltorio de escritorio para el kiosko PartyBox, hecho con **Tauri v2**: una
ventana que es solo un *webview* apuntado a una URL, y que puede ocupar toda la
pantalla ("modo kiosko") **sin ninguna barra** (ni de título, ni de menús, ni
Dock). Proyecto independiente, con su propio git, como `PartyBox`,
`partybox-backend` y `partybox-admin`.

## Qué hace

1. Arranca en una pantalla local **"Conectando…"** y comprueba cada 3 s si el
   servidor de la URL responde. Cuando responde, navega a él. Así el kiosko puede
   arrancar antes que el servidor sin que nadie tenga que tocar nada.
2. Muestra la URL en pantalla completa nativa, sin decoraciones, sin menú, sin
   menú contextual ni zoom (pellizco / Ctrl+rueda). En macOS oculta también el
   Dock y la barra de menús.
3. **Solo permite navegar al mismo origen** que la URL configurada (y a su
   propia pantalla local): un enlace de la página no puede sacar al kiosko a
   otro sitio.
4. **En desarrollo** añade un menú nativo *Kiosco* con atajos (ver abajo). En
   compilaciones de producción no hay menú ni forma de salir del modo kiosko.

## Requisitos

- Rust (`rustup`) y Tauri CLI v2 (`cargo install tauri-cli --version "^2"`,
  comprobado con `tauri-cli 2.0.4`).
- **Toolchain**: las dependencias actuales de Tauri necesitan Cargo ≥ 1.85, así
  que `rust-toolchain.toml` fija **Rust 1.90 solo para este proyecto**; `rustup`
  lo instala la primera vez y el toolchain por defecto del sistema no se toca.
- macOS: Xcode Command Line Tools. Linux: `webkit2gtk-4.1` y compañía
  ([requisitos de Tauri](https://v2.tauri.app/start/prerequisites/)).
- No hay Node ni frontend que compilar: la única página propia es `ui/index.html`.

## Uso en desarrollo

```bash
./run-dev.sh                        # muestra http://localhost:5173 (la app kiosko de PartyBox)
./run-dev.sh http://localhost:5174  # otra URL
./run-dev.sh --windowed             # empieza como ventana normal en vez de en modo kiosko
```

(Equivale a `cd src-tauri && cargo tauri dev -- -- --url <url>`.) También vale
la variable de entorno `PARTYBOX_URL`.

### Atajos (solo en desarrollo)

| Atajo | Acción |
|---|---|
| **⌘ ⇧ K** (Ctrl+Shift+K en Windows/Linux) | **Entrar / salir del modo kiosko** |
| ⌘ R | Recargar la página |
| ⌘ ⇧ H | Volver a la URL inicial |
| ⌘ Q | Salir de la app |

Están también en el menú **Kiosco** (visible cuando no estás en modo kiosko).
Los atajos siguen funcionando con el modo kiosko activo aunque el menú esté
oculto. Por defecto **empieza en modo kiosko**; con `--windowed` empieza como
ventana.

## Opciones de la línea de comandos

| Opción | Efecto |
|---|---|
| `--url <url>` | Página a mostrar. También `PARTYBOX_URL`. Por defecto `http://localhost:5173`. Si falta el esquema se asume `http://`. |
| `--windowed` | Arranca como ventana normal (por defecto arranca en modo kiosko). |
| `--hide-cursor` | Oculta el puntero del ratón en modo kiosko (pantallas táctiles). |

## Compilar para producción

```bash
cd src-tauri && cargo tauri build
```

Genera la app (`.app`/`.dmg` en macOS, `.deb`/`.AppImage` en Linux…) en
`src-tauri/target/release/bundle/`. La versión de producción **no tiene menú ni
atajos**: arranca en kiosko y no hay salida (cerrar el proceso desde fuera, o
`--windowed` para pruebas). Ejemplo de arranque en el mini-PC:

```bash
partybox-shell --url http://localhost:5173 --hide-cursor
```

## Comprobar el modo kiosko sin teclado (autotest)

Solo en compilaciones de depuración: con `PARTYBOX_SELFTEST=1` la app espera a
que cargue la página, ejecuta **la misma función que el atajo** (entra y sale del
modo kiosko) e imprime el estado de la ventana (URL, decoraciones, tamaño frente
al del monitor) antes de cerrarse:

```bash
cd src-tauri && cargo build
PARTYBOX_SELFTEST=1 ./target/debug/partybox-shell --url http://localhost:5173 --windowed
# selftest [after 1st toggle] ... decorated=Some(false) outer=(2560, 1440) monitor=(2560, 1440)
```

Los tests de la regla de navegación: `cd src-tauri && cargo test`.

## Cámara (`getUserMedia`)

`src-tauri/Info.plist` declara `NSCameraUsageDescription`; sin ello macOS ni
pregunta ni concede la cámara. La primera vez saldrá el aviso de permiso del
sistema. Además, **`getUserMedia` solo funciona en contextos seguros**: la URL
debe ser `https://…` o `http://localhost` (una IP de la LAN por `http` no vale).

Los permisos de cámara de macOS se conceden **a una app empaquetada**, no a un
binario suelto lanzado desde el terminal. Para probar la cámara de verdad
empaqueta y abre la `.app`:

```bash
cd src-tauri && cargo tauri build --debug --bundles app
open -a "target/debug/bundle/macos/PartyBox Shell.app" --args --url http://localhost:5173 --windowed
```

## Limitaciones a tener en cuenta

- **Es el motor web del sistema** (WKWebView en macOS, WebKitGTK en Linux,
  WebView2 en Windows), no Chromium. Comportamientos como la cámara, las
  animaciones o el *service worker* pueden diferir de Chromium; **el modo
  offline de la PWA de PartyBox** (service worker) no está garantizado en
  WKWebView, hay que comprobarlo. Si es imprescindible, la alternativa es
  Electron (lleva Chromium dentro).
- Si el servidor cae **después** de cargar la página, el webview se queda con
  lo que tenía (o en blanco al recargar); la pantalla "Conectando…" solo cubre
  el arranque.
- Los atajos de desarrollo son aceleradores de menú nativos; en Linux con
  Wayland/algunos gestores de ventanas el menú se comporta distinto.

## Estructura

```
ui/index.html            pantalla local "Conectando…"
src-tauri/src/main.rs    toda la lógica (argumentos, modo kiosko, atajos, bloqueo de navegación)
src-tauri/tauri.conf.json, capabilities/, Info.plist, icons/
run-dev.sh               atajo para desarrollo
```
