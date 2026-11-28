# Analisi delle prestazioni interattive (28/11/2026)

> Documento di lavoro dell'analisi, conservato com'era, con le misure
> sbagliate **segnate e non cancellate**. Le correzioni e le ipotesi
> escluse sono riassunte in [README.md](README.md). Versione analizzata:
> 6f6f94f (l'ultima prima della diagnostica).


Obiettivo: un'interazione con la sensazione di AutoCAD. Durante Sposta, una
maniglia trascinata o la navigazione, la geometria resta vincolata al
cursore, senza ritardo né rincorsa, e senza sacrificare precisione, snap,
correttezza BIM, Annulla/Esc/Cancel, indipendenza dei progetti e integrità
dei file. Questo progetto si affianca al piano di stabilizzazione già
concordato e non lo sostituisce.

Fase 1 (analisi) e misure del 28/11/2026, sulla versione 6f6f94f.

## Come sono state fatte le misure

- Banco di prova senza interfaccia: il vero `show_plan` (o il fotogramma
  completo `frame_body`, sotto Xvfb) a cui si mandano eventi reali di
  puntatore e clic, sulla casa di esempio (640 elementi) e sulla casa piccola,
  con 10–10.000 linee aggiunte.
- Build `--profile fast` (quasi release), 4 core virtuali del contenitore di
  sviluppo, una prova alla volta. I tempi assoluti sul PC Windows
  dell'utente saranno diversi (in `docs/PERFORMANCE.md` i tempi della pianta
  sono 10–16 ms). Contano i rapporti.
- Fonti dei tempi parziali: i contatori `Perf` già presenti nel programma
  (`CREABIM_PERF=1 CREABIM_PERF_MS=0`).
- La GPU, la presentazione a schermo e la vista 3D non si possono misurare
  qui: per quelle valgono solo la lettura del codice e le misure da fare sul
  PC dell'utente.

## 1. Il percorso mouse → modello → schermo (oggi)

1. Il sistema operativo manda il movimento a winit ed eframe. egui raccoglie
   tutti gli eventi arrivati tra un fotogramma e l'altro e usa solo l'ultima
   posizione del puntatore: **gli eventi non si accumulano** (verificato:
   modello di input di egui).
2. Un fotogramma (`frame_body`, thread dell'interfaccia): pannelli,
   proprietà, elenco, poi ogni area di vista (`show_plan`, `show_3d`).
3. Durante Sposta, per ogni fotogramma (`Modal::update` → `step` →
   `LiveEdit::update`), verificato nel codice:
   - toglie il passo provvisorio del fotogramma prima (`discard_step`: applica
     l'inverso della transazione);
   - controlla gli elementi di partenza;
   - calcola le giunzioni che seguono (`joins::follow`);
   - crea una nuova transazione e la registra (`commit` → `apply_settled`:
     linee dei muri, tipi, quote bloccate): **nuova revisione del modello a
     ogni fotogramma**.
4. Ogni revisione invalida i disegni: la pianta in cui si lavora viene
   **ricostruita per intero sul thread dell'interfaccia**
   (`drawing_live` → `view_drawing_with`). Sezioni, prospetti e scena 3D
   vanno a thread in sottofondo (`builds.rs`), e intanto le loro aree
   mostrano l'ultimo risultato finito. Durante il trascinamento di una
   maniglia, ogni area di pianta si ricostruisce sul posto.
5. `show_plan` chiede un altro fotogramma quando la revisione è cambiata
   (`app.rs:7486`). Poiché lo spostamento riapplica la modifica a ogni
   fotogramma, anche col mouse fermo, il programma gira **in un ciclo continuo
   a piena CPU** finché lo spostamento non finisce.
6. Vista 3D: la scena di una revisione si costruisce in sottofondo
   (`shared_scene`, l'intero modello); quando arriva, il thread
   dell'interfaccia ricostruisce tutti i vertici e li ricarica sulla GPU
   (`rebuild_geometry`, "3d rebuild"). L'oggetto spostato in 3D compare
   quindi con alcune scene di ritardo: è la **rincorsa vista in 3D**
   (comportamento voluto per non bloccare l'interfaccia, `app.rs`
   `rebuild_geometry`).
7. Disegno a schermo: egui triangola le forme (0,3–1,4 ms) e le passa a wgpu.
   Configurazione di presentazione: quella predefinita di egui-wgpu,
   `HIGH_THROUGHPUT` = VSync con **fino a 2 fotogrammi in coda**
   (`desired_maximum_frame_latency: Some(2)`); Volumen non la cambia
   (verificato in `main.rs` e nel sorgente di egui-wgpu 0.36).

## 2. Colli di bottiglia confermati (misurati)

| Scenario (casa di esempio salvo dove detto) | Tempo CPU per fotogramma, mediana (p95) |
|---|---|
| Solo puntatore, niente in corso | 1,0–1,7 ms |
| Pan con il tasto centrale | 0,65 ms |
| Rotellina | 0,5 ms (5,6) |
| Elastico del muro (punto in attesa) | 0,7 ms |
| Selezione a finestra | 0,85 ms |
| **Sposta 1 muro** | **17–19 ms (27–29)** |
| Sposta 1 muro, mouse fermo | 15–22 ms (si ricalcola comunque) |
| Sposta 13 muri | 19 ms (23) |
| Trascina un muro | 20 ms (21) |
| Sposta 10 / 100 / 1.000 linee **[ERRATO, vedi "Correzioni" in README: in realtà 1 linea spostata in una pianta con N linee]** | 16 / 23 / 23 ms (20 / 30 / 31) |
| **Sposta 10.000 linee** **[ERRATO, vedi "Correzioni" in README: in realtà 1 linea spostata in una pianta con N linee]** | **142 ms (196)**, circa 7 fotogrammi al secondo |
| Casa piccola + 10.000 linee **[ERRATO, vedi "Correzioni" in README: in realtà 1 linea spostata in una pianta con N linee]** | 106 ms (145) |
| Fotogramma completo, modello fermo | 5,5 ms |
| Fotogramma completo, modello che cambia | 23–25 ms (27) |

- **C-1:** la ricostruzione completa della pianta a ogni fotogramma è circa
  l'86% del tempo di Sposta (15,7 ms su 17). Cresce con tutto ciò che c'è
  nella pianta, non con ciò che si sposta: 13–19 ms per la casa, 122–137 ms
  con 10.000 linee.
- **C-2:** il ciclo continuo durante Sposta. La modifica si riapplica e la
  pianta si ricostruisce anche col mouse fermo (78 ricostruzioni su 80
  fotogrammi). CPU sempre occupata, sezioni e 3D in sottofondo che
  ripartono di continuo.
- **C-3:** le operazioni che non toccano il modello costano meno di 1 ms: la
  lentezza viene dalla modifica provvisoria del modello e da ciò che ne
  dipende, non da egui né dalla gestione del mouse.
- **C-4:** la modifica provvisoria in sé (`LiveEdit::update`) costa poco
  fino a 1.000 elementi (≤ 1,4 ms), 14–15 ms (fino a 43) con 10.000 linee.
- **C-5:** il pannello delle proprietà costa circa 4 ms a ogni fotogramma,
  anche col modello fermo.

## 3. Colli di bottiglia sospettati (da misurare sul PC dell'utente)

- **S-1:** la coda di presentazione a 2 fotogrammi (`HIGH_THROUGHPUT`)
  aggiunge fino a 33 ms a 60 Hz. Dal codice, non misurato.
- **S-2:** in 3D, la ricostruzione completa della scena (26–75 ms qui) più
  la ricostruzione e il ricaricamento di tutti i vertici a ogni nuova scena.
  Il "3d rebuild" esiste come contatore `Perf` ma qui non si può misurare
  (niente GPU).
- **S-3:** più aree di pianta aperte durante il trascinamento di una
  maniglia: ognuna si ricostruisce sul posto (dal codice).
- **S-4:** thread in sottofondo che competono per i core col thread
  dell'interfaccia (sezioni e scena 3D ricostruite di continuo durante
  Sposta, per C-2).
- **S-5:** copie del disegno a ogni ricostruzione (`drawing` clona il
  `Drawing` per metterlo in cache), costo stimato 1–2 ms.
- **S-6:** il mirino disegnato dal programma (impostazione Crosshair) segue
  lo stesso ritardo dei fotogrammi, mentre il cursore di Windows è disegnato
  subito dal sistema: la distanza tra i due rende visibile il ritardo.

## 4. Misure che mancano

- Tempi reali sul PC dell'utente, con la sua GPU e il suo monitor (frequenza
  di aggiornamento).
- Tempo GPU e tempo di presentazione (coda VSync), con PresentMon su Windows.
- Latenza reale mouse → schermo: si misura solo dall'esterno (vedi sotto).
- Costo del "3d rebuild" e del caricamento sulla GPU nella vista 3D.
- Costo con più aree aperte (pianta + sezione + 3D) durante Sposta.
- Modelli grandi veri (più livelli, migliaia di elementi BIM).

## 5. Parti già efficienti

- Input: nessun accumulo, solo l'ultima posizione del puntatore.
- Pan, zoom, elastico, selezione a finestra: sotto 1 ms di CPU.
- Snap durante Sposta: 0,2 ms; la tabella degli snap si ricostruisce una
  volta sola (1,4–2,3 ms).
- Disegni pesanti e scena 3D già in sottofondo, che non bloccano mai il
  fotogramma (A6).
- Una sola scena 3D condivisa per revisione; spigoli 3D in cache per
  contenuto.
- Bordi sfumati tolti durante pan e zoom (meno triangoli).
- Programma fermo = CPU quasi a zero (A6).
- Contatori `Perf` e tabella dei tempi lenti nel registro attività: una
  base pronta per la modalità diagnostica.

## 6. Interventi con i maggiori benefici (stima)

| # | Intervento | Beneficio atteso | Complessità |
|---|---|---|---|
| O-1 | Non riapplicare la modifica quando il bersaglio non è cambiato (mouse fermo, stesso punto agganciato) | Niente più ciclo continuo; CPU libera; sezioni e 3D si stabilizzano | bassa |
| O-2 | Coda di presentazione a 1 fotogramma (`LOW_LATENCY`), dopo misura sul PC dell'utente | −1 fotogramma di latenza (circa −16 ms a 60 Hz) | bassa (configurazione), da provare su DX12/Vulkan/OpenGL |
| O-3 | Anteprima sovrapposta in pianta (A6-1.1): durante lo spostamento il disegno resta fermo e si ridisegnano solo gli elementi che si muovono, trasformati; il modello si aggiorna alla conferma (o a bassa frequenza per le giunzioni) | Sposta 1 muro da circa 17 ms a 1–3 ms; 10.000 linee da 142 ms a pochi ms **[la misura di partenza era errata: vedi README]** | media-alta |
| O-4 | Anteprima sovrapposta in 3D: gli oggetti spostati disegnati con una matrice di trasformazione sulla GPU, senza aspettare la scena nuova | Oggetto 3D agganciato al cursore | media-alta |
| O-5 | Ricostruzione della pianta solo per gli elementi cambiati e i loro vicini (A6-2.4) | Ogni modifica (anche a fine Sposta) più rapida | alta |
| O-6 | Pannello proprietà ricalcolato solo quando cambia ciò che mostra | −4 ms per fotogramma, sempre | media |
| O-7 | Niente copia del disegno alla messa in cache (`Arc` condiviso) | −1/2 ms per ricostruzione | bassa |

Ordine suggerito per beneficio e rischio: O-1, O-2, O-7, O-6, poi O-3, O-4,
O-5.

## 7. Rischi di regressione per intervento

- **O-1:** nessuna nuova revisione col mouse fermo. Rischi: un cambio non
  dovuto al puntatore (tasto Ctrl per la copia, valore digitato, asse
  bloccato, snap che cambia) deve continuare a riapplicare. Annulla, Esc e
  Ctrl+Z (V-01) restano uguali. Test: le prove di Sposta e i test V-01 e C1.
- **O-2:** su alcune GPU o backend (OpenGL) `Some(1)` può dimezzare gli FPS
  o dare scatti. Va reso regolabile e misurato prima di cambiare il valore
  predefinito.
- **O-3:** l'anteprima potrebbe non mostrare le giunzioni aggiornate o i
  tagli dei muri durante il movimento (si vedrebbero alla conferma). Gli snap
  devono usare le posizioni dell'anteprima. Cambia il funzionamento di
  LiveEdit: le garanzie di V-01, V-10 e V-11 vanno preservate e ritestate.
  Il risultato confermato deve essere identico a oggi (stesso modello,
  stesso disegno: confrontabile con gli hash di contenuto dell'analisi
  retrospettiva).
- **O-4:** come O-3 per il 3D, più il percorso GPU (OpenGL compreso, regole
  di CLAUDE.md su `render3d.rs`).
- **O-5:** rischio di disegni incompleti o vecchi (giunzioni, aperture,
  stanze dipendono dai vicini). Serve un test che confronti sempre la
  ricostruzione parziale con quella completa.
- **O-6:** proprietà non aggiornate (valori vecchi mostrati).
- **O-7:** basso.

## 8. Piano di lavoro incrementale (ogni passo da autorizzare)

1. **P-0, misure e diagnostica (nessun cambiamento di comportamento):** banco
   di prova permanente (sezione 10) e modalità diagnostica (sezione 9).
   Misure sul PC dell'utente (sezione 11) come base.
2. **P-1:** O-1 (fine del ciclo continuo) e O-7.
3. **P-2:** O-2, solo dopo le misure con PresentMon, come impostazione
   prima che come valore predefinito.
4. **P-3:** O-6.
5. **P-4:** O-3, l'anteprima sovrapposta in pianta, prima per Sposta e
   trascinamento.
6. **P-5:** O-4, l'anteprima in 3D.
7. **P-6:** O-5, se le misure mostrano ancora ricostruzioni lente alla
   conferma.

Per ogni passo:
- misure prima e dopo, nelle stesse condizioni;
- test funzionali e di precisione;
- confronto dei disegni finali con gli hash di contenuto;
- un commit separato, che si può annullare da solo;
- CI e anteprima verificate.

## 9. Modalità diagnostica (proposta)

- **Attivazione:** dal menu Aiuto ("Diagnostica prestazioni") o con una
  scorciatoia; spenta in partenza.
- **Pannello piccolo:**
  - FPS e tempo per fotogramma (mediana, p95, p99, massimo degli ultimi 2 s);
  - grafico a barre degli ultimi 120 fotogrammi;
  - tempo diviso per fase (le voci `Perf` già presenti: pianta, snap,
    proprietà, 3D, pannelli);
  - numero di ricostruzioni di disegni e scene;
  - numero di eventi del mouse per fotogramma;
  - età dell'input: tempo tra l'arrivo dell'evento e l'inizio del
    fotogramma, se eframe lo permette, altrimenti stimato.
- **Rapporto di sessione:** pulsante "Registra 30 s" che salva un file CSV
  o JSON (tempi per fotogramma e per fase, revisioni, ricostruzioni,
  impostazioni di presentazione, GPU e backend) accanto al registro
  attività, da inviare.
- **Costo:** misure già fatte da `Perf` (un `Instant` per sezione), più un
  anello di 600 valori. Trascurabile; il grafico si disegna solo se il
  pannello è aperto.
- **Latenze distinte:**
  - interna, misurabile dal programma: dall'evento arrivato a eframe alla
    consegna del fotogramma alla GPU;
  - reale, mouse fisico → luce sul monitor: solo dall'esterno, con
    PresentMon (tempo fino alla visualizzazione), un video al rallentatore
    del telefono (240 fps) che inquadri mouse e schermo, o un analizzatore
    hardware.
- **Tempo GPU:** con le timestamp query di wgpu dove il backend le
  supporta; altrimenti "non disponibile".

## 10. Benchmark permanenti (proposta)

- **Banco di prova ufficiale** nell'app (come `gesture_tests`), lanciato a
  mano in build ottimizzata, disattivato nei test ordinari. Scenari:
  1. Sposta 10, 100, 1.000 e 10.000 linee;
  2. Sposta gruppi di elementi BIM (1 muro, un piano, un edificio);
  3. disegno di linee e polilinee (elastico);
  4. trascinamento di maniglie;
  5. pan e zoom;
  6. selezione a finestra;
  7. snap (tabella e ricerca);
  8. Copia, Ruota, Scala;
  9. orbita 3D (solo su PC con GPU);
  10. spostamento 3D (solo su PC con GPU);
  11. anteprima di inserimento di porte e finestre.

  Modelli: casa piccola, casa di esempio, un modello grande generato (più
  livelli, migliaia di elementi BIM, 10.000 linee).
- **Uscita:** CSV con mediana, p95 e massimo per fotogramma e per fase.
  Lo storico va in `docs/`, una riga per misura (commit, macchina, data).
- **Budget:** nella CI solo come allarme per peggioramenti grossi (×2),
  perché le macchine della CI sono rumorose. Il confronto fine si fa sulla
  stessa macchina, prima e dopo.
- **Equivalenza:** per ogni ottimizzazione che salta del lavoro (anteprima,
  ricostruzione parziale), un test che confronta il risultato con quello del
  percorso completo (modello e disegno, con gli hash di contenuto).
- **Procedura Windows per l'utente:**
  1. avviare con la diagnostica accesa;
  2. aprire la casa di esempio;
  3. eseguire gli scenari 1, 2, 4, 5, 9 e 10 a mano per 30 s ciascuno;
  4. salvare il rapporto;
  5. ripetere con PresentMon in registrazione;
  6. facoltativo: video a 240 fps.

## 11. Informazioni necessarie sul PC dell'utente

- Processore (modello, core), memoria.
- Scheda grafica (modello) e versione del driver; più schede grafiche?
- Monitor: risoluzione, frequenza di aggiornamento (60/120/144 Hz),
  ridimensionamento di Windows (100/125/150%), più monitor?
- Versione di Windows.
- Mouse: frequenza di polling, se nota.
- Il backend grafico usato da Volumen (si legge nel registro attività
  all'avvio, Aiuto → registro).
- Dimensioni tipiche dei progetti (elementi, livelli, linee DXF importate).
- Se durante il lavoro sono aperte più aree (pianta + 3D + sezioni).
- Se il mirino (Crosshair) è acceso.

## Obiettivi realistici

- Sposta e trascinamenti in pianta, casa di esempio: oggi circa 17–20 ms di
  CPU per fotogramma qui; obiettivo ≤ 8 ms dopo O-1 e O-3 (60 fps stabili con
  margine).
- 10.000 linee: oggi circa 140 ms **[errato: 140 ms è 1 linea spostata in una pianta di 10.000; con 10.000 spostate sono 2,2–2,4 s, vedi C-7]**; obiettivo ≤ 16 ms con l'anteprima
  sovrapposta.
- Mouse fermo durante Sposta: nessun lavoro (oggi un fotogramma pieno a
  ripetizione).
- Latenza complessiva a 60 Hz: circa 1 fotogramma di CPU, più 1 di
  presentazione, più la scansione dello schermo. Con O-1, O-2 e O-3,
  20–35 ms sono realistici a 60 Hz e meno a 120–144 Hz. Sotto i 20 ms a
  60 Hz è difficile per qualunque programma con VSync. Da confermare con
  le misure.
- 3D: oggetto spostato agganciato al cursore entro 1 fotogramma (O-4).
  Oggi è in ritardo di 2–5 fotogrammi (stima dal codice e dai tempi della
  scena).

## Macchina di riferimento dell'utente (28/11/2026)

- Intel Core i7-13620H (10 core, 16 thread), 32 GB RAM a 2667 MT/s, SSD
  NVMe, Windows 11 Pro 26H2.
- Solo GPU integrata Intel UHD Graphics, memoria condivisa (fino a
  15,9 GB), driver 32.0.101.7088, DirectX 12, Feature Level 12.1.
- Monitor BenQ PD2700U 27": 3840×2160, **29,97 Hz**, HDR attivo, 10 bit.

L'hardware non basta a spiegare il problema: AutoCAD, sulla stessa
macchina, è nettamente più reattivo.

## Analisi specifica per questa configurazione

### Frequenza a 29,97 Hz, VSync e fotogrammi in coda (dal codice; misura con PresentMon)

- Un intervallo di aggiornamento dura 33,4 ms.
- Volumen presenta in VSync (FIFO) con **fino a 2 fotogrammi in coda**
  (`HIGH_THROUGHPUT`, predefinito di egui-wgpu, non cambiato).
- Durante Sposta il programma produce fotogrammi di continuo (C-2) più in
  fretta dello schermo (16–20 ms contro 33,4): **la coda si riempie e resta
  piena**. Ogni immagine mostrata è quindi vecchia di circa 2–3 intervalli:
  stima 70–100 ms dal movimento alla luce sul monitor.
- Il cursore di Windows è un overlay disegnato dal sistema alla posizione
  più recente, a ogni aggiornamento, indipendente dalla coda di Volumen. La
  geometria resta indietro di 2–3 intervalli rispetto al cursore; quando il
  mouse si ferma, la coda si svuota in 70–100 ms e la geometria "raggiunge"
  il cursore. **È esattamente l'effetto descritto.**
- Un programma che disegna solo quando serve e con coda a 1 fotogramma
  (comportamento tipico dei CAD) mostra al massimo 1 intervallo di ritardo.
- A 60 Hz tutti questi tempi si dimezzano. Il problema resta, ma è meno
  visibile.

### HDR attivo (ipotesi da verificare)

Con l'HDR di Windows, un programma SDR come Volumen (superficie sRGB a
8 bit) viene composto da Windows nello spazio HDR. Questo può impedire la
presentazione diretta ("independent flip") e aggiungere un passaggio di
composizione. Si verifica con PresentMon (colonna PresentMode: "Composed
Flip" o "Hardware Independent Flip"), con HDR acceso e spento.

### Risoluzione 4K e scalatura (misurato sulla CPU)

| Scenario | 1600×1000 @100% | 3840×2160 @150% | 3840×2160 @200% |
|---|---|---|---|
| Sposta 1 muro, mediana (p95) | 19 ms (25) | 20 ms (35) | 16 ms (20) |
| Vertici dell'interfaccia | 37.000 | 51.000 | 49.000 |
| Triangolazione | 0,4 ms | 0,55 ms | 0,5 ms |
| Pan | 0,54 ms | 0,70 ms | 0,57 ms |
| Rotellina, p95 (max) | 4 ms (13) | 22–31 ms (68–73) | 16 ms (87) |

- Sul lato CPU il 4K pesa poco: il costo dominante resta la ricostruzione
  della pianta (C-1).
- Sul lato GPU, egui ridisegna l'intera finestra a ogni fotogramma, a 4K:
  circa 8,3 milioni di pixel per fotogramma, con la banda di memoria
  condivisa con il processore. Non misurabile qui.

### C-6 (nuovo, confermato): atlante dei caratteri durante lo zoom

- A ogni nuova dimensione del testo in pianta (quote, etichette, note)
  egui rasterizza i glifi e l'atlante dei caratteri cresce: misurati
  aggiornamenti da 0,26 a **16,7 milioni di pixel (texture 4096×4096,
  circa 64 MB)**.
- Ogni crescita ricarica l'intero atlante sulla GPU in un solo fotogramma,
  che costa 30–73 ms di CPU qui, più il trasferimento: pesante su una GPU
  integrata.
- Più alta è la scalatura, più grandi i glifi in pixel e più grande
  l'atlante.
- Proposta (O-8): dimensioni del testo arrotondate a pochi gradini durante
  lo zoom, poi esatte da fermo; oppure testo disegnato come forme
  vettoriali o come mesh scalate. Rischio: qualità o nitidezza del testo
  durante lo zoom, da verificare.

### Vista 3D (dal codice; misura sul PC)

- **Ridisegno continuo:** ridisegnata a ogni fotogramma dell'interfaccia,
  anche con camera e modello fermi (nessuna immagine in cache). Durante
  Sposta (C-2) gira dunque di continuo.
- **Risoluzione:** quella fisica (a 4K anche 2.500×1.700 pixel o più per
  la sola vista), con multicampionamento 4× sul colore e sulla profondità.
- **Illuminazione:** ombre filtrate su 3×3 punti e 8 letture delle mappe
  del cielo per pixel quando ombre e occlusione sono attive (White model e
  Maquette sempre).
- **A ogni scena nuova** (di continuo durante uno spostamento) si rifanno
  la mappa del sole e le 8 mappe del cielo da 1024×1024, e tutti i vertici
  vengono ricostruiti e ricaricati sulla GPU.
- La texture del 3D è passata a egui senza copie
  (`register_native_texture`): questo è efficiente.
- Proposte (O-9):
  - non ridisegnare il 3D se camera, scena e illuminazione non sono
    cambiate;
  - non rifare le mappe del cielo durante il trascinamento (rifarle alla
    conferma);
  - valutare il multicampionamento 2× o la risoluzione ridotta *solo
    durante il movimento*, con la qualità piena da fermo.

  Rischio: va verificato che l'immagine finale resti identica.

### Cursore di sistema e mirino

- Il cursore di Windows segue sempre la posizione reale.
- Il mirino di Volumen (impostazione Crosshair) è disegnato dentro i
  fotogrammi e ha lo stesso ritardo della geometria: con il mirino acceso
  il ritardo si vede su tutto lo schermo.

### Nuovo ordine suggerito per questa configurazione

1. **P-0, misure sul PC dell'utente:** PresentMon, Task Manager (GPU),
   diagnostica, 30 Hz e 60 Hz, HDR acceso e spento.
2. **O-1:** niente fotogrammi inutili; da solo dovrebbe impedire alla coda
   di restare piena col mouse fermo.
3. **O-2:** coda a 1 fotogramma, regolabile; su questa macchina il
   beneficio atteso è circa 33 ms a 30 Hz.
4. **O-9:** niente ridisegni del 3D quando nulla cambia.
5. **O-8:** atlante dei caratteri.
6. **O-3, O-4:** anteprima sovrapposta, il vero passo verso la sensazione
   di AutoCAD.

## Aggiornamento 28/11 (seconda analisi): prove a 60 Hz senza HDR, il ritardo resta

L'utente ha portato il monitor a 60 Hz (HDMI, 3840×2160), HDR spento, mouse
CSL Vertical Mouse (Bluetooth o 2,4 GHz). AutoCAD resta reattivo, Volumen
no: il problema è nel software.

### Correzione di una misura precedente

Nel primo banco, "Sposta N linee" aggiungeva le linee alla selezione una
alla volta con un'opzione che ogni volta svuotava la selezione: si spostava
in realtà **1 linea** in una pianta con N linee. Le misure corrette sono
quelle qui sotto.

### C-7 (nuovo, causa principale con molte linee): esclusione degli snap ricalcolata a ogni fotogramma

- `Modal::snap_exclude` (`modal.rs:913`), chiamata da `planview::show` a
  ogni fotogramma durante Sposta, chiama `Model::dependents(id)`
  (`model.rs:142`) per **ogni elemento selezionato**.
- `dependents` scorre **tutto il modello** e costruisce per ogni elemento
  l'elenco dei suoi riferimenti. Costo: selezionati × elementi del modello.
- L'elenco non cambia durante il movimento: gli snap in quella fase sono
  congelati (`freezes_snaps`).
- Misure con 10.000 linee in pianta e 1–10.000 spostate (snap spenti):

| Spostate | Oggi | Esclusione calcolata una volta (prova diagnostica isolata) |
|---|---|---|
| 1 | 143–148 ms | 141 ms |
| 1.000 | 352–387 ms | 174–193 ms |
| 10.000 | 2.169–2.393 ms | 367–558 ms |

- Con gli snap accesi i numeri sono simili (gli snap sono congelati).
- Ipotesi esclusa con una prova: la verifica di selezione `Selection::contains`
  (ricerca lineare) **non** è la causa; con una ricerca a costo costante i
  tempi non cambiano.

### Altri costi verificati nel codice (secondari)

- `joins::follow` (`joins.rs:1420`) ricostruisce l'elenco di tutti i muri
  collegabili a ogni fotogramma.
- `Snapper::hidden` (`snaps.rs:752`) esclude gli elementi scorrendo tutto
  l'elenco degli esclusi per ogni candidato.
- `drawing_print` (`planview.rs:5454`) calcola l'impronta di tutti gli
  elementi disegnati a ogni fotogramma: misurato **0,08 ms** con 11.600
  elementi, trascurabile.
- I lavori in sottofondo fanno una copia completa del modello per revisione
  (`builds.rs`, `snapshot`): misurato 0,5 ms (casa) e 2,5 ms (10.000 linee).
  Ogni lavoro finito chiede un nuovo fotogramma.
- `apply_settled` (registrazione della modifica): `walls::settle` e
  `holds::settle` guardano il modello per muri e quote bloccate.
- Il backend reale si legge nel registro attività (`app.rs:402`: scheda,
  backend, driver). Predefinito di egui-wgpu: tutti i backend principali
  più OpenGL, preferenza "alte prestazioni".

### Il mouse

Un mouse Bluetooth o 2,4 GHz può aggiungere qualche millisecondo (polling a
125 Hz = fino a 8 ms; Bluetooth spesso di più), ma vale anche per AutoCAD:
non spiega la differenza.

### Nuovo intervento O-10 (beneficio alto, rischio basso)

Calcolare l'esclusione degli snap una volta sola, all'inizio della fase di
movimento, e tenerla per tutto lo spostamento. Il comportamento resta
identico, perché gli snap in quella fase sono già congelati. Test: gli
stessi elementi esclusi di oggi in ogni fase (Base, Reference, Follow,
Duplica), più i test esistenti di Sposta e V-01.
