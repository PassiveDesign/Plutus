# Fonts

The app is set in two typefaces, both under the SIL Open Font License 1.1,
which allows them to be bundled with the app and kept in a public repository.

- **IBM Plex Sans** for names, labels and sentences. Licence in
  `OFL-IBMPlexSans.txt`.
- **JetBrains Mono** for paths, times, figures and patterns, so columns of
  them line up. Licence in `OFL-JetBrainsMono.txt`.

The files are taken unmodified from the upstream releases:

| File here | Upstream file |
| --- | --- |
| `IBMPlexSans-Regular.woff2` | `fonts/complete/woff2/IBMPlexSans-Regular.woff2` |
| `IBMPlexSans-Medium.woff2` | `fonts/complete/woff2/IBMPlexSans-Medium.woff2` |
| `IBMPlexSans-SemiBold.woff2` | `fonts/complete/woff2/IBMPlexSans-SemiBold.woff2` |
| `IBMPlexSans-Bold.woff2` | `fonts/complete/woff2/IBMPlexSans-Bold.woff2` |
| `JetBrainsMono-Regular.woff2` | `fonts/webfonts/JetBrainsMono-Regular.woff2` |
| `JetBrainsMono-Medium.woff2` | `fonts/webfonts/JetBrainsMono-Medium.woff2` |
| `JetBrainsMono-Bold.woff2` | `fonts/webfonts/JetBrainsMono-Bold.woff2` |

Sources:

- IBM Plex Sans: the `@ibm/plex-sans` package, version 2.0.0, from
  <https://github.com/IBM/plex>. The "complete" files cover Latin, Cyrillic,
  Greek and the other scripts Plex Sans has, so file names in those scripts
  are drawn in the same face.
- JetBrains Mono: release 2.304 from
  <https://github.com/JetBrains/JetBrainsMono/releases/tag/v2.304>.
