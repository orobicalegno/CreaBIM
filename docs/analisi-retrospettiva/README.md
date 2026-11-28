# Analisi retrospettiva di Volumen (25–26 novembre 2026)

Analisi delle ultime **10 anteprime con cambiamenti al programma** più l'anteprima
attuale (11 versioni), con la versione 0620165 come riferimento e, per alcuni
difetti, il CreaBIM originale (26cc49c). Solo analisi: nessuna correzione è stata
fatta durante questo lavoro.

Lo stato di ogni problema (aperto, in correzione, corretto, con il commit) si segue
in [`docs/regressioni.md`](../regressioni.md), sezione *Analisi retrospettiva*.
Questo documento è il rapporto come è stato consegnato, più il modo di ripetere le
verifiche.

Contenuto della cartella:

| Cartella | Cosa contiene |
|---|---|
| `script/` | gli script dei confronti tra versioni (viste SVG, stili, IFC, PDF) e di confronto delle immagini |
| `prove/` | le prove esplorative (test Rust temporanei) usate per riprodurre i problemi |
| `risultati/` | i risultati dei confronti, in testo (hash di contenuto, tempi, esiti) |
| `immagini/` | due confronti visivi tra versioni (azzurro: solo prima; rosso: solo dopo) |

Nessun eseguibile, cache o progetto pesante è conservato qui: si ricreano con i
passi in fondo.

---

## 1. Come sono state scelte le versioni

Criterio approvato dall'utente: le ultime 10 anteprime **effettivamente pubblicate**
il cui codice del programma cambia rispetto all'anteprima precedente, più l'attuale.
Escluse quelle che cambiano solo test, documentazione o CI (tenendo traccia di
compilatore e dipendenze).

Fonti: le release `volumen-anteprima-*`, i log dei job "Public preview download"
della CI (riga *Published …* o URL della release), la cronologia Git.

| # | Commit | Pubblicata (UTC) | Contenuto | Eseguibile |
|---|---|---|---|---|
| 0 | 1584697 | 25/11 17:30 | C3 (Cancel solo delle impostazioni della finestra) e documentazione | originale verificato |
| 1 | 369e1ed | 25/11 16:59 | C2: catene di muri e travi | originale verificato |
| 2 | e778fe1 | 25/11 16:32 | C1: Ctrl+Z con uno strumento a metà | originale verificato |
| 3 | b364c5a | 23/11 19:00 | Cancel solo dei passi della finestra; Annulla non ripetuto | ricompilato |
| 4 | e9f8376 | 23/11 18:45 | DXF lontano accanto al modello | ricompilato |
| 5 | 0bc2f49 | 23/11 17:28 | Maquette; giunti tra facce uguali tolti | ricompilato |
| 6 | 8fc7008 | 23/11 08:55 | White model: sole, ombre, occlusione | ricompilato |
| 7 | d92e600 | 22/11 13:08 | White model in piante, sezioni, viste 3D salvate | ricompilato |
| 8 | ac485f0 | 22/11 12:05 | Ombre 3D su ogni scheda grafica, errori nel registro | ricompilato |
| 9 | 57a6d13 | 21/11 20:04 | Ombre, occlusione, export ad alta risoluzione | ricompilato |
| 10 | 3e54ea8 | 21/11 17:27 | Tetti a falde non tagliati dai muri; clic destro a piedi | ricompilato |

- Escluse: 2b339c1, 5d7c606, 8c742a4 (solo test), 1e2f428 (Rust fissato a 1.100.0,
  la stessa versione già usata dalla CI), bebf8e5, eefdee0 (CI), 1e8e701
  (`.gitignore`, stesso codice di 57a6d13).
- Compilatore: rustc 1.100.0 in tutte le build (log CI). `Cargo.lock` invariato da
  0620165 all'attuale.
- Le build della CI partono dal *commit di unione* della PR con il ramo di base
  (31de310, mai cambiato): per tutte le 11 versioni l'albero dei file del commit di
  unione è identico a quello del commit del ramo.
- Eseguibili originali: per le versioni 0–2 (copie datate) controllati sha256 e il
  commit di unione scritto dentro `Volumen.exe`
  (`risultati/exe-originali-sha256.txt`). Per le versioni 3–10 i file pubblicati non
  esistono più (il vecchio script cancellava e ricreava l'anteprima) e gli artefatti
  CI, che scadono il 21/02/2027, non erano scaricabili dalla rete dell'ambiente di
  analisi: è stata ricompilata la riga di comando dal sorgente certo.
- C3 (20d0699) non è mai stata pubblicata da sola: lo script salta una build se
  nella PR c'è già un commit più nuovo; il suo codice è nella 1584697.

## 2. Sintesi

- 18 problemi più un comportamento da definire (V-04): 2 P1, 6 P2, 5 P3, 5 M.
- Nessuna regressione grave introdotta nelle 11 versioni: i due P1 nascono prima
  (V-01 nel CreaBIM originale, V-10 in Volumen il 19/11).
- Unica regressione introdotta nell'intervallo: V-07 (progetti con White model o
  Maquette dichiarati "danneggiati" dalle versioni precedenti).
- Stabile o migliorato:
  - 3e54ea8 ha tolto linee e campiture spurie sotto i tetti a falde (difetto
    presente in 0620165);
  - dopo 3e54ea8 le 59 viste dei 5 campioni sono identiche nel contenuto in tutte
    le versioni;
  - Linee nascoste, Ombreggiato e Wireframe non sono mai stati toccati dalle
    modifiche a White model e Maquette;
  - IFC identico nel contenuto in tutte le versioni;
  - sezioni, prospetti e 3D più veloci del 30–45%;
  - nessun test rimosso o indebolito (56 aggiunti).

## 3. Registro dei problemi

Per stato, test e commit di correzione vedere `docs/regressioni.md`. Qui le
evidenze raccolte.

### P1

**V-01 – Ctrl+Z durante uno spostamento non confermato cancella dalla cronologia lo
spostamento confermato prima.**
- Certezza: confermato con riproduzione (flusso reale di Sposta). Riprodotto
  identico su 0620165 e sul CreaBIM originale 26cc49c (`prove/q8b_minimal.rs`).
- Origine: CreaBIM originale (bc1c2a6, 10/10/2026; 3a652ee).
- Riproduzione: sposta un muro (A, confermato); sposta di nuovo lo stesso muro e,
  prima di cliccare il punto finale, Ctrl+Z; muovi il mouse; Esc.
- Atteso: Ctrl+Z annulla solo lo spostamento in corso; A resta annullabile.
- Effettivo: il muro resta dove l'ha messo A ma A non è più nella cronologia;
  annullando tutto il muro non torna al punto di partenza; il modello non è uguale a
  nessuno stato confermato (i muri collegati non coincidono più); la barra di stato
  dice "Undid: Move GF South".
- Causa: `modal.rs`, `LiveEdit::cancel` riconosce il proprio passo provvisorio dal
  nome (`undo_label() == label`) e lo toglie con `discard_last`; Ctrl+Z durante
  un'operazione in corso arriva alla cronologia (C1 non copre le operazioni dal
  vivo); il passo successivo scarta allora il passo confermato con lo stesso nome.
  `Op::Replace` non verifica lo stato di partenza (`ops.rs`).
- Stesso rischio (dal codice): trascinamento delle maniglie dell'intero elemento
  (`handles.rs`, nome "Move …"), push/pull, Grip.
- Test esistenti: nessuno lo copriva.

**V-10 – Passare a un altro progetto durante uno spostamento cancella lavoro
confermato dell'altro progetto.**
- Certezza: confermato con riproduzione (prova Q9; il cambio di progetto era
  simulato con `keep_open` e sostituzione del documento, non dal menu).
- Origine: 771a39c (19/11/2026, più progetti aperti, Ctrl+Tab); non esiste nel
  CreaBIM originale.
- Effettivo: se l'ultimo passo del progetto B ha lo stesso nome (tipico in una copia
  "Salva con nome"), il fotogramma successivo lo scarta: passi di B da 1 a 0, nulla
  da ripristinare.
- Causa: `openprojects.rs` (`bring_in`) e `app.rs` (`load_model`) non chiudono
  l'operazione in corso; poi lo stesso riconoscimento per nome di V-01.

### P2

**V-02 – Cancel nelle Impostazioni: torna il valore memorizzato, non l'effetto
applicato.** Confermato (prova Q1): tema applicato Solarized e memorizzato Sage;
maniglie applicate Edges e memorizzate Quiet. Dal codice: anche dimensioni in
modalità modifica e posizione della barra. Origine 7cb4176 (19/11); C3 non l'ha
introdotto né corretto.

**V-07 – Progetti con White model o Maquette dichiarati danneggiati dalle versioni
precedenti.** Confermato: con 0620165 e 57a6d13 "the project file is damaged: bad
JSON: unknown variant `White`"; con d92e600 e l'attuale si apre. Il numero di
formato (`FORMAT_VERSION`) è fermo a 1 dall'ottobre 2026. Introdotto da d92e600 e
0bc2f49.

**S-02 – Clic al centro della porta d'ingresso: viene presa la linea di griglia.**
Confermato (prova di selezione). Coerente con l'ordine di selezione attuale,
contrario all'orientamento deciso (costruttivi prima dei riferimenti).

**S-03 – Finestre sopra il piano di taglio non selezionabili con un clic.**
Confermato: "WC window" e "Store window" (davanzale 1,50 m, taglio 1,20 m) sono
disegnate in pianta, ma il clic al centro o sul telaio prende il muro. Origine non
identificata (codice di selezione invariato nelle 11 versioni).

**V-06 – IFC esportato e reimportato: i muri attaccati al tetto tornano più alti.**
Confermato (somma altezze del campione da 72,4 a 85,9 m; avviso "clipping ignored"
mostrato). Preesistente (crate IFC invariato). Griglie, cavedi, sito e parti persi:
già in `docs/KNOWN_GAPS.md`.

**T-01 – L'indipendenza degli stili non è protetta da test.** Guasto simulato
(`joints_for` falso per tutti gli stili): Ombreggiato e Linee nascoste perdono 54
linee nel prospetto sud; nessun test di core, geom e spec fallisce (test dell'app
non eseguiti con il guasto).

### P3

- **V-03** – Canc e Annulla: elementi identici ma rimessi in coda all'elenco
  (`IndexMap::shift_remove` + inserimento in coda). Origine: nucleo del CreaBIM
  originale. Confermato.
- **V-08** – C3 non copre tutto: finestra UCS, caselle "UCS icon" e "at its origin"
  e cronologia degli UCS precedenti restano dopo Cancel. Dal codice. Preesistente.
- **V-11** – Il cambio di progetto non annulla il punto in attesa né la catena: il
  clic successivo nell'altro progetto parte da lì. Dal codice.
- **V-13** – IFC: GUID di IfcProject, IfcSite e IfcBuilding nuovi a ogni
  esportazione (quelli degli elementi sono stabili). Confermato. Preesistente.
- **V-14** – Il test dei budget di tempo (`creabim-geom/tests/perf.rs`, disattivato)
  fallisce sia in 0620165 sia nell'attuale (prospetti Est e Ovest 170–177 ms > 160).

### M (miglioramenti)

- **V-05** – Scheda grafica incompatibile: nessun messaggio e nessun registro
  (`main.rs`: `run_native` restituisce un errore, nessuna console in release, il
  registro attività parte solo in `CreaBimApp::new`). Dal codice; già noto.
- **V-09** – Esportazione SVG non deterministica nell'ordine e nel verso dei
  segmenti (geometria identica).
- **V-12** – Esportazioni attuali: 11,1% di polilinee di lunghezza zero, 22,6%
  duplicate (152 375 polilinee in 59 viste).
- **S-01** – La finestra Informazioni mostra il commit di unione della PR, non
  quello del ramo.
- **T-02** – La correzione DXF di e9f8376 non ha un test; la prova Q10 passa.

### Da definire (non è un difetto)

- **V-04** – Ctrl+Y con il primo punto di un muro in attesa rifà il passo annullato
  e lascia il punto in attesa (prova Q2).

## 4. Comportamenti cambiati di proposito nelle 11 versioni

- 3e54ea8: esplorando a piedi, clic destro sposta, Maiusc+destro mira, centrale
  gira la testa.
- 0bc2f49: White model e Maquette non disegnano i giunti tra facce uguali.
- b364c5a: UNDO e REDO non si ripetono con Invio o clic destro.
- C1, C2, C3: vedere `docs/regressioni.md`.

## 5. Cosa è stato controllato in ciascuna versione

| Controllo | 0 | 1–2 | 3–10 | 0620165 | 26cc49c |
|---|---|---|---|---|---|
| Eseguibile originale verificato | sì | sì | no | – | – |
| Sorgente = commit compilato in CI | sì | sì | sì | – | – |
| Riga di comando ricompilata | sì | sì | sì | sì | – |
| 59 viste SVG di 5 campioni | sì | sì | sì | sì | – |
| 5 stili × 10 viste | sì | sì | sì (White da d92e600, Maquette da 0bc2f49) | sì | – |
| IFC (contenuto) e PDF (solo generazione) | sì | sì | sì | sì | – |
| Apertura file tra versioni | sì | – | d92e600, 57a6d13 | sì | – |
| Prestazioni (build ottimizzata) | sì | – | – | sì | – |
| Prove nell'app | sì | – | – | solo V-01 | solo V-01 |
| Interfaccia reale, GPU, mouse fisico | no | no | no | no | no |

Dove c'è "no" o "–" l'assenza di problemi trovati non significa assenza di problemi.

Risultati principali dei confronti:
- `risultati/viste-hash-contenuto.tsv`: per ogni vista, hash del contenuto
  normalizzato (ordine e verso dei segmenti) in 12 versioni. Cambia solo in
  3e54ea8 (17 viste).
- Stili (sample-layered, 10 viste): Linee nascoste, Ombreggiato e Wireframe cambiano
  solo in 3e54ea8; White solo in 8fc7008 e 0bc2f49 (voluti; in 0bc2f49 tolti solo i
  segmenti orizzontali dei giunti); Maquette mai cambiato dopo 0bc2f49. (Gli SVG
  dei confronti degli stili non sono conservati: si rigenerano con `styles.sh`.)
- IFC: contenuto identico in tutte le versioni, GUID normalizzati.
- Tempi (build ottimizzata, `cargo test --release -p creabim-geom --test perf --
  --ignored`, stessa macchina, due esecuzioni):

| Vista | 0620165 | 1584697 |
|---|---|---|
| Scena 3D | 38 ms | 30 ms |
| Piante | 16 / 14 / 10 / 1 ms | 16 / 14 / 9 / 1 ms |
| Section A-A / B-B | 145 / 100 ms | 96 / 43 ms |
| Prospetti | 230–280 ms | 130–177 ms |
| 3D Street / Garden | 375 ms | 260 ms |

## 6. Piano di correzione (approvato il 26/11 nell'ordine seguente)

1. V-01, V-10, V-11 (gruppo P1) – autorizzato.
2. V-02 e V-08: completamento di Cancel.
3. V-04: decidere Ripristina durante un'operazione non confermata.
4. T-01: test permanenti dell'indipendenza degli stili.
5. T-02: test permanente dell'import DXF.
6. V-07: compatibilità e migrazione dei file (prima un piano).
7. V-06 e V-13: IFC.
8. S-02 e S-03: selezione.
9. Gli altri problemi registrati.

## 7. Collaudo manuale proposto (Windows, mouse, GPU vera)

1. V-01: sposta un muro e conferma; sposta di nuovo lo stesso muro, Ctrl+Z prima
   del punto finale, Esc; Ctrl+Z più volte. Atteso: il muro torna al punto di
   partenza.
2. V-10: apri un progetto e una sua copia; nella copia sposta un muro;
   nell'originale avvia uno spostamento, Ctrl+Tab, muovi il mouse, torna alla
   copia, Ctrl+Z. Atteso: lo spostamento della copia è ancora lì.
3. V-02: Impostazioni, cambia tema, Cancel. Atteso: torna subito il tema precedente.
4. C3: apri Impostazioni, F3 nella pianta, Cancel. Atteso: F3 resta come l'hai messo.
5. C1: solaio di 3 vertici, Ctrl+Z due volte. Atteso: tolti due vertici, il lavoro
   precedente resta.
6. C2: catena di 3 muri, Ctrl+Z due volte, Ctrl+Y, clic. Atteso: la catena riparte
   dal punto giusto (anche muri ad arco e travi).
7. S-03: clic sulla finestra alta del WC in pianta (oggi prende il muro).
8. V-07: salva un progetto con una vista White model e aprilo con l'anteprima del
   21/11 (oggi: "damaged").
9. White model e Maquette in 3D con ombre accese e spente; rotellina in tutte le
   viste comprese le assonometriche; nel registro attività cercare "3D shadows off".
10. Import di un DXF in coordinate di rilievo: deve comparire accanto al modello,
    nella pianta aperta.
11. Salva, chiudi, riapri: progetto identico, Ctrl+Z ancora disponibile.
12. Su un PC con una scheda grafica vecchia: annotare cosa succede all'avvio.

## 8. Limiti

- Verificato eseguendo: dati e salvataggi, esportazioni, stili, IFC, prestazioni,
  prove nell'app per V-01, V-02, V-03, V-10, S-02, S-03, DXF.
- Solo dal codice: V-05, V-11, V-08, il caso delle maniglie in V-01, la coerenza
  2D/3D delle cache (legate alla revisione globale del modello).
- Non verificato: interfaccia reale, GPU, focus, webview dei menu; contenuto dei
  PDF; snap, ortogonale, polare, inserimento numerico (codice invariato nelle 11
  versioni); test dell'app con il guasto agli stili; eseguibili originali 3–10.

---

## 9. Come ripetere le verifiche

Tutto in una cartella di lavoro fuori dal repository e in *worktree* Git
temporanei, mai nella copia principale. Esempio:

```bash
export ANALISI=$HOME/volumen-analisi
mkdir -p $ANALISI/bin $ANALISI/cmp $ANALISI/styles
```

Compilare la riga di comando di ogni versione (una cartella di build per tutte;
`CARGO_PROFILE_DEV_DEBUG=0` la tiene piccola):

```bash
git worktree add --detach $ANALISI/wt 0620165
cd $ANALISI/wt
for v in 0620165 3e54ea8 57a6d13 ac485f0 d92e600 8fc7008 0bc2f49 e9f8376 b364c5a e778fe1 369e1ed 1584697; do
  git checkout -q --detach $v
  CARGO_TARGET_DIR=$ANALISI/target CARGO_PROFILE_DEV_DEBUG=0 cargo build -p creabim-cli
  cp $ANALISI/target/debug/creabim $ANALISI/bin/creabim-$v
done
```

Modelli di partenza: generati una volta con la versione di riferimento e usati da
tutte (gli identificativi cambiano a ogni generazione):

```bash
cd $ANALISI/cmp
for s in sample sample-layered sample-typed sample-furnished sample-garden; do
  $ANALISI/bin/creabim-0620165 $s old-$s.json
  $ANALISI/bin/creabim-1584697 views old-$s.json | grep -v '^schedule' | sed -E 's/^[a-z0-9]+ +[0-9:]+ +//' > views-$s.txt
done
```

Confronti (viste, IFC, PDF; poi stili su `sample-layered` con tutte le viste
impostate a uno stile):

```bash
bash docs/analisi-retrospettiva/script/matrix.sh
cd $ANALISI/styles
for st in HiddenLine Shaded Wireframe White Maquette; do
  sed -E "s/\"style\": \"(HiddenLine|Shaded|Wireframe)\"/\"style\": \"$st\"/g" ../cmp/old-sample-layered.json > layered-$st.json
done
bash docs/analisi-retrospettiva/script/styles.sh
```

Gli SVG si confrontano con `script/canon.py` (contenuto normalizzato: ordine dei
tag, verso dei segmenti, punto di partenza dei poligoni), `script/svgcmp.py`
(differenze per tipo) e `script/render.js` + `script/pixdiff.py` (immagini:
azzurro solo prima, rosso solo dopo; richiedono Playwright e Pillow).

Le prove esplorative (`prove/`) sono test Rust temporanei da copiare in un worktree,
mai nel progetto:
- `probe_data.rs` → `crates/creabim-store/tests/` (salvataggio, cronologia, NaN);
- `probe_ifc.rs` → `crates/creabim-ifc/tests/` (IFC esporta e reimporta);
- `probe_app.rs` e `probe-app-tests.diff` → in coda a
  `crates/creabim-app/src/undo_tests.rs` e `gesture_tests.rs` della versione
  1584697 (prove Q1, Q2, Q4, Q8, Q8b, Q9, Q10, selezione);
- `q8b_minimal.rs` → come `crates/creabim-app/src/undo_tests.rs` di una versione
  vecchia, aggiungendo in fondo ad `app.rs`
  `#[cfg(test)] #[path = "undo_tests.rs"] mod undo_tests;` (usato per 0620165 e
  26cc49c).

Lancio: `cargo test -p creabim-app --bin creabim-app probe_ -- --nocapture`.
