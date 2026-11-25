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

### Bug noti, con il loro test disattivato

Ogni bug ha un test che descrive il comportamento **corretto**: oggi fallisce,
quindi è disattivato (`#[ignore]`) perché la suite resti verde. Va riattivato
(togliendo la riga `#[ignore]`) nello stesso commit della correzione. Per
vederli fallire adesso:

```bash
cargo test -p creabim-app --bin creabim-app known_bug -- --ignored
```

| Bug | Test disattivato | Causa |
|---|---|---|
| Cancel nelle 4 finestre di impostazioni rimette anche le impostazioni cambiate altrove mentre era aperta | `undo_tests::known_bug_cancel_in_a_settings_window_keeps_settings_changed_elsewhere` | All'apertura viene fotografato **tutto** l'insieme delle impostazioni e Cancel lo rimette intero (`osnapui.rs`, `dialog_end`); per le finestre del progetto invece si tiene conto di ciò che ha cambiato la finestra stessa |

Osservato durante l'analisi: l'elenco "uno strumento ha qualcosa in corso"
usato da Esc e dal clic destro (`PlanState::has_input`) non comprende il
percorso dello strumento Parti né la catena di muri disegnata nel 3D.

## Annulla durante uno strumento a metà (C1, corretto)

**Bug corretto.** Con 31 strumenti della pianta, Ctrl+Z dato a metà
disegno (un contorno aperto, il primo punto di un muro, una quota a metà…)
annullava il lavoro fatto **prima** dello strumento: solai, coperture,
cavedi, stanze, controsoffitti, separatori, scale, ringhiere, fondazioni,
estrusioni, sweep, vuoti, terreno, pavimentazioni, sezioni, prospetti,
richiami, camere, quote, etichette, angoli, raggi, diametri, lunghezze
d'arco, note, quote di livello, schizzi, guide, griglie, allineamenti, muri,
travi. Causa: l'Annulla "dentro il comando" conosceva solo LINE/PLINE e i
punti chiesti dai comandi.

**Ora:** se uno strumento ha qualcosa non confermato, Ctrl+Z toglie
l'ultima cosa non confermata (un vertice, un punto, la scelta fatta), un
passo alla volta, e non tocca mai la cronologia del progetto; senza niente
in corso, Ctrl+Z annulla l'ultimo passo come sempre. Esc e il clic destro
non cambiano: tolgono tutto ciò che è in corso, poi lasciano lo strumento
(`PlanState::step_back`); Ctrl+Z (`PlanState::take_back_input`) un passo
alla volta e non lascia lo strumento.

| Comportamento | Test |
|---|---|
| Per **ogni** strumento della pianta, con 1, 2 e 3 clic: a metà, Ctrl+Z non cambia niente di confermato (6 gruppi in parallelo) | `c1_undo_part_way_walls_slabs_roofs_stairs`, `c1_undo_part_way_parts_and_site`, `c1_undo_part_way_rooms_and_openings`, `c1_undo_part_way_views`, `c1_undo_part_way_annotation`, `c1_undo_part_way_drafting` |
| Nessuno strumento resta fuori da quei test | `c1_every_plan_tool_is_in_a_group` |
| Senza niente in corso, Ctrl+Z annulla l'ultimo passo come sempre | `c1_with_nothing_part_way_undo_takes_back_the_last_step_as_ever` |
| Ctrl+Z toglie un vertice alla volta; Esc tutto il contorno, poi lascia lo strumento | `c1_esc_drops_the_whole_outline_and_ctrl_z_only_its_last_corner` |
| Il contorno del solaio e il primo punto del muro (i due casi trovati in G4, test riattivati) | `fixed_c1_undo_while_drawing_a_slab_outline_keeps_earlier_work`, `fixed_c1_undo_after_a_walls_first_click_keeps_earlier_work` |

Verificato: prima della correzione i test falliscono (31 strumenti
elencati); dopo, passano. Con Ctrl+Z fatto agire come Esc (tutto il
contorno) il test dei vertici fallisce. Non verificato: gli strumenti nel
**riquadro 3D** (la catena di muri in 3D è trattata dallo stesso codice, ma
non c'è ancora un test che la disegni) e in **sezione**; il Ripristina
durante uno strumento a metà (da valutare a parte).

Un'eccezione voluta, introdotta da C2: in una catena di muri o di travi i
pezzi già disegnati sono confermati, e Ctrl+Z toglie l'ultimo (solo quello).
Il test dei 6 gruppi l'ammette in modo stretto: un solo passo annullato e
il progetto esattamente com'era prima dell'ultimo clic.

## Annulla e Ripristina in una catena di muri o travi (C2, corretto)

**Bug corretto.** Disegnando muri (o travi) uno dopo l'altro, Ctrl+Z
toglieva l'ultimo muro ma la catena ripartiva dalla fine del muro tolto,
dove non c'era più niente. Causa: lo strumento ricordava solo l'ultimo
punto (`pending`), e Annulla non sapeva che quel punto apparteneva al muro
tolto.

**Ora:** lo strumento tiene l'elenco dei pezzi della catena
(`PlanState::chain_steps`: il passo della cronologia, l'inizio, la fine e,
per le travi, la trave a cui si aggancia). Con la catena in corso e niente
altro a metà, Ctrl+Z toglie l'ultimo pezzo e la catena riparte dal suo
inizio; Ctrl+Z ancora toglie il pezzo prima, e così via fino al primo
punto, poi il primo punto, poi la cronologia come sempre. Ripristina
rimette il pezzo e la catena riparte dalla sua fine. Vale solo se l'ultimo
passo della cronologia è proprio quel pezzo: se nel frattempo è stato
fatto altro (un colore cambiato, un'altra modifica), la catena si chiude e
Ctrl+Z non tocca quel lavoro per sorpresa. Esc, il clic destro e il cambio
di strumento chiudono la catena: dopo, Annulla e Ripristina sono quelli di
sempre.

| Comportamento | Test |
|---|---|
| Muri dritti: Ctrl+Z ripetuto toglie un muro alla volta, il progetto ogni volta identico a prima, e la catena riparte dal punto giusto; poi il primo punto; nessun passo precedente annullato | `c2_undo_in_a_chain_of_straight_walls_steps_back_a_wall_at_a_time` |
| Lo stesso con i muri ad arco | `c2_undo_in_a_chain_of_arc_walls_steps_back_an_arc_at_a_time` |
| Lo stesso con le travi | `c2_undo_in_a_chain_of_beams_steps_back_a_beam_at_a_time` |
| Ripristina (muri e travi) rimette il pezzo, la catena riparte dalla sua fine e si può continuare | `c2_redo_in_a_chain_puts_the_piece_back_and_the_chain_goes_on_from_its_end` |
| Dopo Ctrl+Z la trave nuova si aggancia alla trave su cui la catena è rimasta (giunzione nello stesso passo) | `c2_a_beam_drawn_after_undo_joins_the_beam_the_chain_stands_on` |
| Arco a metà (fine cliccata, non ancora curvato): Ctrl+Z toglie prima quella, la catena resta | `c2_an_arcs_end_clicked_goes_first_and_the_chain_stays` |
| Catena interrotta da un'altra modifica: Ctrl+Z non la annulla per sorpresa | `c2_a_chain_interrupted_by_other_work_never_undoes_that_work_by_surprise` |
| Dopo Esc o un altro strumento, Annulla e Ripristina come sempre | `c2_after_esc_or_another_tool_ctrl_z_is_the_historys_again` |
| Il progetto, salvato e riaperto dopo Ctrl+Z nella catena, è identico | `c2_a_chain_stepped_back_saves_and_reopens_as_it_is` |
| La pianta disegnata (stratigrafie e giunzioni d'angolo comprese) è la stessa di prima dell'ultimo muro | `c2_the_walls_left_meet_as_they_did_before_the_last_one` |
| Il caso trovato in G4 (test riattivato) | `fixed_c2_undo_in_a_wall_chain_resumes_from_the_previous_point` |

Verificato: prima della correzione 5 dei primi 6 test falliscono; in una
copia isolata, togliendo la parte di Annulla falliscono 8 test su 11 (i 3
che passano sono controlli di sicurezza, veri anche prima), togliendo
quella di Ripristina fallisce il test di Ripristina. Coerenza 2D/3D: il
progetto dopo Ctrl+Z è identico elemento per elemento a quello di prima,
e la pianta disegnata è la stessa; il 3D si costruisce dallo stesso
progetto, ma **non** è stato provato a schermo né su una GPU. Non coperta:
la catena di muri disegnata nel **riquadro 3D** (`pending_3d`): lì Ctrl+Z
toglie il punto in corso (C1) e poi annulla come sempre, senza riprendere
la catena.

## Altri comportamenti dubbi emersi dai test

- **Immagini ombreggiate di 2049 pixel.** Le sezioni, i prospetti e le viste
  3D ombreggiate producono un'immagine larga fino a 2049 pixel (un pixel in
  più del previsto, `section.rs`, `DepthBuffer::render`); le ombre delle
  piante fino a 4001. Nel programma il limite reale non viene superato: la
  libreria grafica, aprendo la scheda video, chiede obbligatoriamente 8192
  pixel per lato (egui-wgpu 0.36), e su una scheda che non li ha il programma
  non parte del tutto. Il limite di 2048 è quello predefinito di egui, usato
  nei test. Da correggere comunque per robustezza (proposta a parte).
  **Se la scheda video non supporta 8192 pixel** (verificato leggendo il
  codice di egui-wgpu 0.36 ed eframe 0.36, non su una scheda reale): la
  richiesta di apertura della scheda fallisce, non c'è un secondo tentativo
  con un limite più basso o con un'altra scheda, l'errore arriva a `main`; su
  Windows, dove il programma non ha console, **Volumen non si apre e non
  mostra nessun messaggio**, e non scrive nemmeno nel registro attività.
  Riguarda solo schede molto vecchie: quelle DirectX 11 e successive
  supportano almeno 16384 pixel.
- **Griglia sopra il muro.** Un clic sull'asse di un muro che coincide con
  una linea di griglia seleziona la griglia, non il muro. Ordine attuale e
  proposta: vedi *Selezione degli elementi sovrapposti*.

## Selezione degli elementi sovrapposti

Dove più elementi sono sotto il puntatore, **Tab** passa al successivo e il
contatore accanto al puntatore apre l'elenco di tutti: resta così.

### Ordine attuale (in cui un clic li prende)

- **Viste 2D** (piante, sezioni, prospetti, viste 3D salvate;
  `planview.rs`, `under_pointer` e `hit_all`): prima le annotazioni (quote,
  etichette, note, testi); poi simboli e segni (porte e finestre, teste e
  linee di griglia, marche di sezione e di vista); poi le campiture di ciò
  che è tagliato (muri, solai); poi crop box e riferimenti (DXF, immagini);
  poi ciò che il raggio colpisce nel modello. Solo fra gli elementi di
  disegno 2D (dettagli, note) l'ultimo creato viene per primo.
- **Riquadro 3D** (`app.rs`, `under_3d`): schizzi, teste di griglia e crop
  box; poi ciò che si colpisce dal più vicino; le linee di griglia solo se
  non si colpisce nient'altro.

### Proposta (non ancora applicata)

Nelle viste 2D, gruppi in quest'ordine: elementi costruttivi (muri, solai,
tetti, pilastri, travi, scale, ringhiere, porte, finestre, arredi); disegno
2D e annotazioni; riferimenti (griglie, livelli, guide, linee di sezione,
DXF e immagini, crop box); locali. Nello stesso gruppo l'ultimo creato per
primo, con due eccezioni: porte e finestre prima del muro che le ospita, e
un clic proprio sulla testa di una griglia (il cerchio con la lettera)
prende la griglia. Nel 3D nessun cambiamento. Da decidere: le annotazioni
prima o dopo gli elementi costruttivi.

## Bug già corretti e protetti

| Bug | Test |
|---|---|
| Annulla ripetuto da Invio o da un clic destro breve | `editui::tests::undo_never_repeats_on_enter_or_a_quick_right_click` |
| Il Cancel di una finestra annullava anche il lavoro fatto altrove | `editui::tests::cancel_in_a_window_never_takes_back_work_done_elsewhere` |
| Le pubblicazioni dell'anteprima cancellavano quella buona | `.github/scripts/test-publish-preview.sh` |
