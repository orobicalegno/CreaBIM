# Registro dei comportamenti protetti

Ogni comportamento qui sotto è **protetto da un test**: se una modifica lo
rompe, la CI si ferma e l'anteprima non viene sostituita. Ogni test è stato
provato contro il guasto che deve intercettare (il codice rotto apposta, in
una copia isolata del repository: vedi la colonna *Verificato contro*).

I test dei gesti sono in `crates/creabim-app/src/gesture_tests.rs` e si
lanciano con:

```bash
cargo test -p creabim-app --bin creabim-app gesture_tests
```

Tre categorie, sempre separate:

- **Corretto e voluto**: deciso, protetto dai test.
- **Attuale, forse sbagliato**: è il comportamento di oggi, ma potrebbe
  essere un guasto. **Non** è fissato nei test finché non viene deciso.
- **Da definire**: manca una decisione; nessun test lo fissa.

## Rotellina (G1)

### Corretto e voluto (protetto)

| Comportamento | Test | Verificato contro |
|---|---|---|
| Uno scatto della rotellina avanti ingrandisce, indietro rimpicciolisce della stessa quantità; non sposta la vista | `g1_wheel::a_wheel_notch_zooms_forward_in_and_back_out_and_never_pans` | M1 |
| Con lo **scorrimento fluido** (lo scatto arriva diviso in tanti piccoli passi) lo zoom è lo stesso di uno scatto intero, indipendente dal mouse | `g1_wheel::smooth_scrolling_zooms_as_much_as_a_whole_notch`, `g1_wheel::a_notch_split_by_smooth_scrolling_zooms_a_pane_as_much_as_a_whole_one` | M1, M5 |
| Ctrl + rotellina zooma una volta sola, non due | `g1_wheel::ctrl_and_the_wheel_zoom_once_not_twice` | M4 |
| Lo zoom resta centrato sul punto sotto il puntatore, e avanti + indietro torna esattamente dov'era: in **pianta**, **sezione**, **prospetto** e **vista 3D salvata** in un riquadro 2D | `g1_wheel::the_wheel_zooms_about_the_pointer_in_a_plan` / `_in_a_section` / `_in_an_elevation` / `_in_a_saved_3d_view` | M1, M2, M6 |
| La rotellina su un altro punto dello schermo non cambia il riquadro | `g1_wheel::the_wheel_away_from_a_pane_leaves_it_as_it_is` | M3 |

Banco di prova (G0): `g0_bench::the_bench_opens_a_view_and_a_click_on_a_wall_selects_it`
(un clic dentro un muro lo seleziona, un clic nel vuoto deseleziona;
verificato contro M0).

### Attuale, forse sbagliato (non fissato)

- **Scorrimento "a pixel" trattato come touchpad.** Se il sistema manda lo
  scorrimento in pixel invece che in righe, Volumen sposta la vista invece di
  zoomare. Su **Windows** questo non succede con un mouse, nemmeno con lo
  scorrimento fluido: Windows manda sempre "righe", anche frazionarie
  (verificato nel codice di winit 0.30.13, la libreria che riceve gli eventi
  di Windows; ed è protetto dai test sopra). Può succedere invece su **macOS**
  e su **Linux con Wayland**, dove i mouse a scorrimento fluido mandano pixel
  come un touchpad: lì la rotellina potrebbe spostare invece di zoomare.

### Da definire

- Scorrimento **laterale** (rotellina inclinabile, touchpad di lato): oggi
  sposta la vista.
- **Touchpad di Windows**: lo scorrimento verticale a due dita arriva come
  una rotellina e quindi zooma (Windows non permette di distinguerli); lo
  spostamento laterale sposta.
- Scorrimento "a pagine" (raro): oggi vale 10 scatti.
- La rotellina nel **riquadro 3D** non è ancora testabile senza una piccola
  modifica al programma (intervento G7, da autorizzare).

## Annulla, Ripristina e Cancel delle finestre (G4)

I test sono in `crates/creabim-app/src/undo_tests.rs`
(`cargo test -p creabim-app --bin creabim-app undo_tests`). Lavorano sulla
casa di partenza (stratigrafie, materiali, porte e finestre nei muri) e
confrontano **tutto il progetto**, elemento per elemento con ogni sua
proprietà e nel suo ordine, non il numero degli elementi. Le modifiche di
prova toccano geometria (un muro spostato), stratigrafie (uno strato più
spesso), materiali (un colore), proprietà (altezza e nome di un muro).

### Corretto e voluto (protetto)

| Comportamento | Test | Verificato contro |
|---|---|---|
| Annulla toglie solo l'ultimo passo, mai quelli prima | `undo_takes_back_the_last_step_and_nothing_before_it` | U1 |
| Ripristina ricostruisce esattamente ogni stato; avanti e indietro più volte, sempre gli stessi stati | `redo_rebuilds_each_state_exactly` | U1, U2 |
| Una modifica nuova dopo Annulla sostituisce ciò che si poteva ripristinare | `a_new_change_after_undo_replaces_what_could_be_redone` | U3 |
| Annulla dentro PLINE toglie l'ultimo punto, non il lavoro fatto prima | `undo_in_a_polyline_takes_back_its_last_point_and_no_earlier_work` | U8 |
| Cancel di una finestra annulla solo la sua modifica e conserva quella fatta prima di aprirla (9 finestre del progetto) | `cancel_takes_back_the_windows_own_change_and_keeps_what_came_before` | U1, U4, U5, U7 |
| Cancel senza modifiche nella finestra non cambia niente (13 finestre) | `cancel_with_nothing_changed_in_the_window_changes_nothing` | U4 |
| Cancel non annulla il lavoro fatto altrove mentre la finestra è aperta (9 finestre) | `cancel_never_takes_back_work_done_elsewhere_while_the_window_is_open` | U4 |
| Save conferma la modifica; resta un passo normale che Annulla toglie da solo (9 finestre) | `save_keeps_the_windows_change_as_an_undo_step` | U1, U6, U9 |
| Aprire e chiudere una finestra molte volte (con Save e con Cancel) non cambia il progetto né i passi di Annulla (13 finestre) | `opening_and_closing_a_window_again_and_again_leaves_the_project_as_it_was` | U4, U9 |

Le 9 finestre del progetto: Materials, Assemblies, Types, Levels,
Annotation styles, Sheet templates, Units, Mark numbering, Graphic styles.
Le 4 finestre di impostazioni: Settings, Drafting settings, Layers, UCS. Per
queste ultime Save e Cancel sono decisi dentro la finestra stessa: i test
chiamano la funzione che i loro pulsanti attivano (`dialog_end`), non il
clic sul pulsante.

Guasti simulati (in una copia isolata, uno alla volta): U1 Annulla toglie
due passi; U2 Ripristina non riapplica; U3 una nuova modifica non svuota
Ripristina; U4 il vecchio Cancel che annullava tutto; U5 Cancel non annulla
niente; U6 Save si comporta come Cancel; U7 la finestra non riconosce le
proprie modifiche; U8 Annulla in PLINE annulla il comando precedente; U9
Save annulla l'ultimo passo.

### Problemi trovati (da correggere, non fissati nei test)

- **Annulla durante il contorno di un solaio annulla il lavoro precedente.**
  Con due punti del contorno già cliccati, Ctrl+Z ha annullato la modifica
  fatta prima (una stratigrafia), e i punti del solaio sono rimasti.
  Annulla "dentro il comando" vale oggi solo per LINE, PLINE e i comandi che
  chiedono punti, non per il contorno dei solai.
- **Annulla durante una catena di muri.** Ogni muro della catena è confermato
  subito; Ctrl+Z toglie correttamente l'ultimo muro, ma il disegno riprende
  dalla fine del muro tolto, dove non c'è più niente.
- **Cancel delle finestre di impostazioni** (Settings, Drafting settings,
  Layers, UCS) rimette **tutte** le impostazioni com'erano all'apertura,
  anche quelle cambiate altrove nel frattempo (per esempio lo stile
  Maquette scelto dal menu Render torna a Shaded).

## Altri comportamenti dubbi emersi dai test

- **Immagini ombreggiate di 2049 pixel.** Le sezioni, i prospetti e le viste
  3D ombreggiate producono un'immagine larga fino a 2049 pixel (un pixel in
  più del previsto, `section.rs`, `DepthBuffer::render`); le ombre delle
  piante fino a 4001. Nel programma il limite reale non viene superato: la
  libreria grafica, aprendo la scheda video, chiede obbligatoriamente 8192
  pixel per lato (egui-wgpu 0.36), e su una scheda che non li ha il programma
  non parte del tutto. Il limite di 2048 è quello predefinito di egui, usato
  nei test. Da correggere comunque per robustezza (proposta a parte).
- **Griglia sopra il muro.** Un clic sull'asse di un muro che coincide con
  una linea di griglia seleziona la griglia, non il muro.

## Bug già corretti e protetti

| Bug | Test |
|---|---|
| Annulla ripetuto da Invio o da un clic destro breve | `editui::tests::undo_never_repeats_on_enter_or_a_quick_right_click` |
| Il Cancel di una finestra annullava anche il lavoro fatto altrove | `editui::tests::cancel_in_a_window_never_takes_back_work_done_elsewhere` |
| Le pubblicazioni dell'anteprima cancellavano quella buona | `.github/scripts/test-publish-preview.sh` |
