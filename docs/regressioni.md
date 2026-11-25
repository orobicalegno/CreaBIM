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

## Altri comportamenti dubbi emersi dai test

- **Immagini ombreggiate di 2049 pixel.** Le sezioni, i prospetti e le viste
  3D ombreggiate producono un'immagine larga fino a 2049 pixel. Una scheda
  video che accetta al massimo 2048 pixel per lato (schede vecchie, alcune
  macchine virtuali) farebbe fallire il disegno della vista. Sembra un errore
  di un pixel nel calcolo della dimensione dell'immagine.
- **Griglia sopra il muro.** Un clic sull'asse di un muro che coincide con
  una linea di griglia seleziona la griglia, non il muro.

## Bug già corretti e protetti

| Bug | Test |
|---|---|
| Annulla ripetuto da Invio o da un clic destro breve | `editui::tests::undo_never_repeats_on_enter_or_a_quick_right_click` |
| Il Cancel di una finestra annullava anche il lavoro fatto altrove | `editui::tests::cancel_in_a_window_never_takes_back_work_done_elsewhere` |
| Le pubblicazioni dell'anteprima cancellavano quella buona | `.github/scripts/test-publish-preview.sh` |
