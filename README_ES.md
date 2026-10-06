<p align="center">
  <img src="assets/logos/aural_banner.svg" alt="Aural" width="640">
</p>

<p align="center">
  Un cliente de YouTube Music para escritorio, teléfonos Android y Android TV, escrito en Rust con <a href="https://freyaui.dev">Freya</a>.
</p>

<p align="center">
  <a href="README.md">English</a> · <b>Español</b>
</p>

---

## Capturas

<p align="center">
  <img src="assets/screenshots/player.jpg" alt="Reproductor a pantalla completa con el fondo animado" width="100%">
</p>

| | |
| --- | --- |
| <img src="assets/screenshots/lyrics.jpg" alt="Letras sincronizadas"> | <img src="assets/screenshots/queue.jpg" alt="Cola"> |
| Letras sincronizadas | Cola |
| <img src="assets/screenshots/artist.jpg" alt="Página de artista"> | <img src="assets/screenshots/search.jpg" alt="Búsqueda"> |
| Página de artista | Búsqueda |
| <img src="assets/screenshots/library.jpg" alt="Canciones que te gustan"> | <img src="assets/screenshots/launcher.jpg" alt="Launcher de Android TV"> |
| Canciones que te gustan | Launcher de Android TV |

## Funciones

- **Tu biblioteca de YouTube Music**: canciones que te gustan, playlists y álbumes, al iniciar sesión con tu cuenta de Google.
- **Inicio** con tu feed de YouTube Music: selección rápida y estantes de álbumes, mezclas y playlists.
- **Búsqueda** de canciones, artistas, álbumes y playlists, con mejor resultado y filtros. En escritorio vive en la barra superior (⌘K / Ctrl K).
- **Páginas de artista** como en YouTube Music: banner, oyentes mensuales, canciones más populares, álbumes, sencillos, playlists y artistas relacionados.
- **Reproducción en streaming** sin cortes entre canciones, con volumen normalizado, aleatorio, repetir y adelantar o retroceder (arrastrando la barra, o izquierda y derecha en el control).
- **Letras sincronizadas**: palabra por palabra cuando existen, con los duetos separados por voz. Se buscan primero en Apple Music y luego en LRCLIB, Musixmatch, NetEase, KuGou y YouTube.
- **Me gusta**: dale o quita me gusta a una canción desde el reproductor, la pantalla completa o su fila, sincronizado con tu cuenta de YouTube Music.
- **Cola a tu medida**: arrastra una canción por su agarradera para moverla, desliza una canción a la izquierda para agregarla a la cola y desliza una de la cola a la izquierda para quitarla.
- **Traducción de letras**: cada línea en español bajo la original, de Apple Music o de las traducciones humanas de Musixmatch, o una traducción automática cuando no hay.
- **Portadas de Apple Music** en toda la app, con la miniatura de YouTube como respaldo.
- **Portadas animadas** (opcionales), buscadas en Apple Music y decodificadas en el dispositivo.
- **Reproductor a pantalla completa** con fondo animado a partir de la portada, cola, y modos de pantalla limpia y nocturno.
- **Luz de portada**: la portada de la canción que suena tiñe suavemente la app.
- **Aural Connect**: reproduce en otro Aural de tu Wi-Fi y contrólalo desde tu celular o computadora, como Spotify Connect. Se empareja una vez con un código de 4 dígitos.
- **Controles de Android**: el reproductor en los ajustes rápidos y la pantalla de bloqueo, y música que sigue sonando en segundo plano.
- **Diseño para celular** táctil: pestañas abajo, mini reproductor y reproductor a pantalla completa vertical.
- **Ajustes**: la foto de tu cuenta, tamaño del texto y de la interfaz.
- **Pensado para el control de la TV**: todo funciona con las flechas, OK y Atrás.

## Plataformas

| Plataforma | Estado |
| --- | --- |
| Android TV (Android 9+, ARM de 32 y 64 bits) | Funciona |
| Teléfonos Android | Funciona |
| macOS | Funciona |
| Windows 10/11 | Compila; el inicio de sesión usa el WebView2 que trae Windows |
| Linux | Compila; el inicio de sesión necesita `webkit2gtk-4.1` (o 4.0) instalado |

## Compilar

Necesitas Rust estable reciente (edición 2024).

### Escritorio

```sh
cargo run --release -p aural
```

### Android

Requisitos: el SDK de Android con NDK, build-tools y CMake, JDK 17, y [`cargo-ndk`](https://github.com/bbqsrc/cargo-ndk):

```sh
cargo install cargo-ndk
rustup target add aarch64-linux-android armv7-linux-androideabi
```

Compila el APK (sin Gradle):

```sh
# Solo ARM de 64 bits (por defecto)
./scripts/android.sh

# Universal: ARM de 64 y 32 bits, para TVs más viejas
./scripts/android.sh arm64-v8a armeabi-v7a
```

El APK queda en `build/aural.apk`, firmado con tu keystore de debug. Instálalo con `adb install -r build/aural.apk`, o cópialo al dispositivo y ábrelo desde un explorador de archivos.

## Controles

| Tecla | Acción |
| --- | --- |
| Flechas / D-pad | Moverse |
| Enter / OK | Seleccionar |
| Esc / Atrás | Regresar |
| ⌘K / Ctrl K | Buscar (escritorio) |

## Estructura

```
crates/aural    la app: interfaz, navegación, motor de reproducción, biblioteca de YouTube Music
crates/lyrics   búsqueda de letras en varios proveedores, con tiempos por palabra
crates/motion   búsqueda de portadas animadas y el decodificador por hardware de Android
crates/webview  ventana nativa de inicio de sesión para macOS, Windows y Linux
crates/aural/src/connect  Aural Connect: descubrimiento, emparejamiento y control remoto
android/        manifest, recursos y los ayudantes en Java (WebView de inicio de sesión, sesión de medios, pantalla encendida)
scripts/        compilación y empaquetado para Android
assets/         fuentes, íconos y logos
```

## Créditos

Aural se apoya en el trabajo de:

- [Sonora](https://github.com/sonorahq): interfaz, proveedores de letras y la ventana de inicio de sesión de escritorio
- [Artwork API](https://github.com/boidushya/artwork.boidu.dev) para portadas animadas
- [ytmusic-rs](https://github.com/sonorahq/ytmusic-rs): API de YouTube Music
- [kawarp](https://github.com/better-lyrics/kawarp): fondo animado
- [Freya](https://github.com/marc2332/freya): framework de interfaz
- [Lucide](https://lucide.dev): íconos

Aural no está afiliado a YouTube, Google, Apple ni Musixmatch.

## Licencia

[GPL-3.0-or-later](https://www.gnu.org/licenses/gpl-3.0.html)
