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
  ([requisitos de Tauri](https://v2.tauri.app/start/prerequisites/)), p. ej. en
  Debian/Ubuntu: `sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`.
- No hay Node ni frontend que compilar: la única página propia es `ui/index.html`.

## Uso en desarrollo

### Desde `dev.sh` (recomendado)

El script de la carpeta padre arranca todo el stack y, si quieres, también esta
app apuntada al kiosko:

```bash
cd ..                                # /Users/admin/Project/partybox
./dev.sh start --shell               # backend + admin + kiosko + este shell
./dev.sh start --shell --kiosk       # ídem, forzando el modo kiosko (pantalla completa)
./dev.sh shell --windowed            # solo el shell (si el kiosko no responde aún, espera en "Conectando…")
./dev.sh shell --rotate=90 --cursor=hide   # panel vertical y sin puntero, sin tocar la URL
./dev.sh shell http://localhost:5174 # el shell apuntado a otra URL (aquí, el admin)
./dev.sh status                      # incluye una línea "shell"
./dev.sh stop                        # para el stack y el shell
```

Sin `--shell`, `./dev.sh` no cambia: no abre ninguna ventana. Las opciones tras
`--shell` (o tras `shell`) son las de abajo; lo que no empiece por `--` se toma
como URL. La primera vez compila Tauri (un par de minutos); después es
inmediato. Sus logs salen mezclados con los demás con el prefijo `[shell]`.

### Directamente

```bash
./run-dev.sh                        # muestra http://localhost:5173 (la app kiosko de PartyBox)
./run-dev.sh http://localhost:5174  # otra URL
./run-dev.sh --windowed             # empieza como ventana normal
./run-dev.sh --rotate=90 --cursor=hide
```

(Equivale a `cd src-tauri && cargo tauri dev -- -- --url <url>`.) También vale
la variable de entorno `PARTYBOX_URL`.

### Atajos (solo en desarrollo)

| Acción | macOS | Linux / Windows |
|---|---|---|
| **Entrar / salir del modo kiosko** | **⌘ ⇧ K** | **Ctrl + Shift + K** |
| Recargar la página | ⌘ R | Ctrl + R |
| Volver a la URL inicial | ⌘ ⇧ H | Ctrl + Shift + H |
| **Herramientas de desarrollo** (inspector, consola, red) | ⌘ ⌥ I | **F12** |
| Salir de la app | ⌘ Q | Ctrl + Q |

Están también en el menú **Kiosco** (visible cuando no estás en modo kiosko).
Los atajos siguen funcionando con el modo kiosko activo aunque el menú esté
oculto. Ver "Dónde y cómo arranca" para saber en qué modo empieza.

Las herramientas de desarrollo tienen su propio atajo porque el menú contextual
de la página (clic derecho → "Inspeccionar") está desactivado a propósito, como
en producción. Solo existen en compilaciones de desarrollo.

> **Linux**: los atajos son aceleradores del menú nativo (GTK). Que sigan
> respondiendo con la barra de menús oculta por el modo kiosko es lo esperado
> por cómo funciona GTK, pero **no está comprobado en un Linux real**. Si en tu
> equipo no responden, sal con `Ctrl+C` en el terminal (o `./dev.sh stop`) y
> arranca con `--windowed`.

### Panel vertical (interfaz girada)

Para un panel táctil horizontal montado en vertical, deja el sistema en horizontal y
pide a la app que gire su interfaz: `./dev.sh shell --rotate=90` (el shell lo añade a la
URL como `?rotate=90`; también vale escribirlo tú en la URL). Detalles y diagnóstico del
táctil en `../docs/guia-de-uso.md`.

## Opciones de la línea de comandos

| Opción | Efecto |
|---|---|
| `--url <url>` | Página a mostrar. También `PARTYBOX_URL`. Por defecto `http://localhost:5173`. Si falta el esquema se asume `http://`. |
| `--rotate=<grados>` | Gira toda la interfaz: `0`, `90`, `180` o `270` (sentido horario). Para un panel montado girado; ver "Panel vertical". Se añade a la URL como `?rotate=`. |
| `--cursor=<hide\|show>` | Oculta o muestra el puntero del ratón. Se añade a la URL como `?cursor=` (lo aplica la página) y, con `hide`, el shell también lo oculta a nivel nativo en modo kiosko. Sin la opción decide la página (oculto en producción, visible en desarrollo). |
| `--hide-cursor` | Igual que `--cursor=hide`. |
| `--kiosk` | Arranca en modo kiosko (pantalla completa, sin barras). |
| `--windowed` | Arranca como ventana normal. |
| `--reset-window` | Olvida la pantalla y posición recordadas (ver abajo). |
| `-h`, `--help` | Muestra esta lista. |

Todas las opciones con valor se pueden escribir `--rotate=90` o `--rotate 90`.
`--rotate` y `--cursor` **sustituyen** al mismo parámetro si la URL ya lo traía
(`--url "http://host/?a=1&rotate=180" --rotate=90` → `?a=1&rotate=90`, el resto se
conserva). Un valor no válido (`--rotate=45`) hace que el shell no arranque y lo diga.

### Dónde y cómo arranca

El shell **recuerda dónde estaba**: cada vez que mueves o redimensionas la ventana, y
cada vez que entras o sales del modo kiosko, guarda posición, tamaño y modo en
`window.json` dentro de su carpeta de configuración (macOS:
`~/Library/Application Support/com.partybox.shell/`; Linux: `~/.config/com.partybox.shell/`).
Al arrancar, sin `--kiosk` ni `--windowed`:

| Situación | Resultado |
|---|---|
| Hay algo recordado y esa pantalla sigue conectada | Abre **en esa misma pantalla**, en el mismo modo (si se cerró en kiosko, vuelve a kiosko sobre ella). |
| Primera vez, o la pantalla recordada ya no está (desenchufada, otra resolución), o el fichero está dañado | **Ventana normal**, nunca kiosko: llévala a la pantalla que toque y entra en kiosko (⌘⇧K / Ctrl+Shift+K en desarrollo; en producción, no hay atajo: arranca una vez con `--kiosk` estando la ventana donde quieras, y se recordará). |
| `--kiosk` / `--windowed` | Manda lo que pidas, en la pantalla recordada si la hay. |

Pasos para una instalación nueva con dos pantallas: arrancar sin opciones → aparece
ventana normal → arrastrarla al panel → entrar en kiosko (o cerrar y volver a abrir con
`--kiosk`). Desde entonces, arranca sola en el panel y en kiosko. Para empezar de cero:
`--reset-window` (o borrar `window.json`). El tamaño y la posición guardados son los del
modo *ventana*: al salir de kiosko vuelve a donde estaba. (En macOS, al salir del modo
kiosko la ventana puede quedar unos píxeles más arriba: es de macOS, no se acumula al
reabrir.)
## Compilar para producción

```bash
cd src-tauri && cargo tauri build
```

Genera la app (`.app`/`.dmg` en macOS, `.deb`/`.AppImage`/`.rpm` en Linux…) en
`src-tauri/target/release/bundle/`. En Linux se puede ejecutar el binario
directamente (`src-tauri/target/release/partybox-shell`) o instalar el `.deb`. La versión de producción **no tiene menú ni
atajos**: en kiosko no hay salida con el ratón (cerrar el proceso desde fuera, o
volver a arrancar con `--windowed`). Ejemplo de arranque en el mini-PC:

```bash
partybox-shell --url http://localhost:5173 --rotate=90 --cursor=hide --kiosk
```

Basta con `--kiosk` la primera vez (con la ventana ya en el panel): las siguientes
veces se puede arrancar sin él, porque recuerda pantalla y modo.

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

**Linux**: no hay diálogo de permisos del sistema como en macOS (el acceso a la
cámara depende de los permisos del dispositivo: tu usuario debe estar en el
grupo `video`), así que funciona igual desde `cargo tauri dev` que empaquetada;
comprueba la cámara con `v4l2-ctl --list-devices`.

**macOS**: los permisos de cámara se conceden **a una app empaquetada**, no a un
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
