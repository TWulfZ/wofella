# wolluf

Training companion for osu!mania **stable**, 7K first (rice + LN). This beta covers the chart library, the pattern engine and the **Label** screen: a playfield with your own osu! skin, your scroll speed and the song's audio, used to label chart sections. Maps you play while wolluf is open can be labelled right after, and **Label → Progress** shows how far your labelling has come.

## Download (Windows 10/11)
Get the latest build from [Releases](https://github.com/TWulfZ/wolluf/releases):
- `wolluf-<version>-portable.exe`: runs without installing. Download a newer one to update; your data stays in `%LOCALAPPDATA%\wolluf`.
- `wolluf-<version>-setup.exe`: installer.

The builds are not code-signed yet, so Windows SmartScreen shows "unknown publisher": click **More info → Run anyway**. wolluf only reads your osu! folder; it never writes there.

To share your labels: in **Label**, open **Playback settings** (the tab on the playfield's left edge) and click **Export labels**. It writes a `.jsonl` file into `%LOCALAPPDATA%\wolluf\exports` and opens that folder. When no window is left to label, the button is on the screen itself.

## Descarga (Windows 10/11)
Descarga la última versión en [Releases](https://github.com/TWulfZ/wolluf/releases):
- `wolluf-<versión>-portable.exe`: se ejecuta sin instalar. Para actualizar, descarga uno más nuevo; tus datos quedan en `%LOCALAPPDATA%\wolluf`.
- `wolluf-<versión>-setup.exe`: instalador.

Los ejecutables aún no están firmados, así que Windows SmartScreen muestra "editor desconocido": haz clic en **Más información → Ejecutar de todas formas**. wolluf solo lee tu carpeta de osu!; nunca escribe en ella.

Para compartir tus etiquetas: en **Etiquetar**, abre **Ajustes de reproducción** (la pestaña en el borde izquierdo del playfield) y haz clic en **Exportar etiquetas**. Guarda un archivo `.jsonl` en `%LOCALAPPDATA%\wolluf\exports` y abre esa carpeta. Cuando no quedan ventanas por etiquetar, el botón aparece en la propia pantalla.

## License
MIT. Third-party credits are in [NOTICE](NOTICE).
