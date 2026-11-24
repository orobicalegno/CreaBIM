# Volumen: istruzioni per lavorare su questo repository

Volumen è un CAD/BIM in Rust derivato da CreaBIM (GPL). Chi lo usa e lo
dirige è un architetto (AutoCAD, Archicad), non un programmatore.

## Comunicazione

- Rispondere **in italiano**, con parole semplici; termini tecnici solo se
  servono, spiegati.
- Dire sempre cosa è stato verificato davvero e cosa no (per esempio: "provato
  solo con il renderer software, non su una vera GPU").
- Prima di modifiche ampie o rischiose, proporre un piano e aspettare
  l'approvazione. Non allargare il lavoro oltre la richiesta.

## Regole di sviluppo (obbligatorie)

1. **Un commit per argomento.** Niente commit che mescolano funzioni diverse.
   Ogni intervento ha il suo commit e una verifica descritta (nel messaggio
   del commit o in `docs/`).
2. **Ogni bug corretto ha un test** che fallisce senza la correzione. Prima di
   chiudere, provare che il test fallisce rimettendo temporaneamente il
   codice sbagliato.
3. **Non eliminare né indebolire test esistenti.** Se un test va cambiato
   perché il comportamento cambia di proposito, dirlo esplicitamente.
4. **Non aggiornare mai da soli i file di riferimento** (immagini dei test
   visivi, file "blessed" con `CREABIM_BLESS`): solo se l'utente lo approva,
   dopo aver visto la differenza.
5. **Non modificare codice che funziona** se non serve al compito.
6. **Controlli prima di ogni push**, gli stessi della CI:

   ```bash
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```

   Se cambia un file di `.github/scripts/`:
   `bash .github/scripts/test-publish-preview.sh`.
7. **Versioni stabili:** mai pubblicarne (tag `v*`, release non anteprima)
   senza l'autorizzazione esplicita dell'utente.
8. **Nessuna operazione distruttiva** su Git o su GitHub (force push,
   cancellazione di rami, tag o release dell'utente) senza autorizzazione.

## Anteprime e CI

- Le PR da rami `volumen-*` pubblicano la pre-release `volumen-anteprima`
  **solo se tutti i controlli passano**, e ne conservano una copia permanente
  datata. Dettagli e verifica: `docs/anteprime.md`.
- Dopo un push sul ramo della PR, controllare l'esito della CI e avvisare
  l'utente quando l'anteprima è pronta (link:
  `https://github.com/orobicalegno/CreaBIM/releases/download/volumen-anteprima/Volumen-windows-x64.zip`).
- La versione di Rust è fissata in `rust-toolchain.toml` e nei workflow: si
  aggiorna solo di proposito, in un commit dedicato, con tutti i controlli
  verdi.

## Struttura

- `crates/creabim-core`: modello, documento, annullamento (`document.rs`),
  stratigrafie, giunzioni, stili delle viste (`styles.rs`).
- `crates/creabim-geom`: geometria e disegni: piante (`plan.rs`,
  `ranges.rs`), sezioni, prospetti e viste 3D software (`section.rs`), scena
  3D (`scene.rs`), giunti tra facce uguali (`flush.rs`).
- `crates/creabim-app`: l'applicazione (egui/eframe, wgpu, menu in webview):
  `app.rs`, `shell.rs` (menu e azioni), `planview.rs` (viste 2D),
  `render3d.rs` (3D su GPU), `cmdline.rs` (riga di comando), `snaps.rs`.
- `crates/creabim-cli`: riga di comando (`creabim view … out.svg`, `pdf`,
  `ifc`, `build`): utile per esportare viste e verificarle senza interfaccia.
- `crates/creabim-spec`: formato JSON del modello; i documenti in `docs/` si
  rigenerano con `CREABIM_BLESS=1 cargo test -p creabim-spec`.

## Aree delicate (guasti già successi: toccarle solo con test)

- **Annulla e finestre con Save/Cancel** (`osnapui.rs`, `dialog_*`,
  `in_dialog`): Cancel deve annullare solo i passi fatti nella finestra,
  mai il lavoro fatto altrove.
- **Ripetizione dei comandi** (`cmdline.rs`): Invio e clic destro breve
  ripetono l'ultimo comando; Annulla e Ripeti non vanno mai ripetuti.
- **Mouse e tastiera**: rotellina in tutte le viste (comprese le
  assonometriche), clic destro, Esc, selezione, snap.
- **3D su GPU** (`render3d.rs`): deve funzionare anche con OpenGL (niente
  `textureLoad` su texture di profondità; formati che ogni backend sa
  disegnare). Gli errori grafici vanno nel registro attività, non devono
  bloccare il programma. La validazione degli shader non equivale a una
  prova su una vera GPU.
- **Geometria**: giunzioni muro–solaio–tetto (`apply_joins`), stratigrafie e
  risvolti; un tetto a falde non va tagliato dai muri.
- **Stili grafici** (Shaded, White model, Maquette…): una modifica a uno stile
  non deve cambiare gli altri.
- **Import DXF**: il disegno deve comparire, nella pianta giusta, visibile.
- **File di progetto**: un progetto salvato deve riaprirsi uguale; le
  versioni precedenti dei file devono continuare ad aprirsi.

## Verifiche visive

- Il renderer software (sezioni, prospetti, viste 3D salvate, fogli, PDF) si
  verifica esportando le viste: `target/debug/creabim view <modello> "<vista>"
  out.svg`.
- L'interfaccia si può provare con Xvfb (`Xvfb :79`, `DISPLAY=:79`,
  `xdotool`, `import` per le schermate). Il 3D qui gira su un renderer
  software (llvmpipe/lavapipe): il comportamento su una GPU vera può
  differire; dirlo sempre.
