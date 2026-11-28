# Prova della fluidità sul proprio PC

Questa prova misura quanto Volumen è fluido sul tuo computer. Non serve
nessuno strumento da programmatore. Ogni registrazione dura 30 secondi e
produce un piccolo file di testo da mandare.

Il file **non contiene** nomi di elementi, testi del progetto né percorsi
delle cartelle. Contiene solo:
- conteggi (quanti muri, quante linee…);
- tempi;
- informazioni sul computer: processore, scheda grafica, driver, schermo.

## Prima di iniziare

1. Scarica l'anteprima:
   <https://github.com/orobicalegno/CreaBIM/releases/download/volumen-anteprima/Volumen-windows-x64.zip>
2. Estrai lo zip in una cartella nuova e avvia `Volumen.exe`.
3. Chiudi gli altri programmi pesanti (browser con molte schede, AutoCAD,
   videochiamate). Collega il portatile alla corrente.

I progetti di prova sono **dentro Volumen**: non devi aprire i tuoi
progetti. Ogni progetto di prova si apre come progetto nuovo e non salva
nulla al posto di un file esistente. Se un tuo progetto è aperto, salvalo
prima.

## Aprire la diagnostica

Menu **Help › Performance diagnostics**. A destra compare un pannello. Per
chiudere la diagnostica basta chiudere il pannello (la X). Quando il pannello
è chiuso, la diagnostica è spenta.

Il pannello mostra in alto lo stato:
- **Idle: no frames needed (normal)** = Volumen è fermo perché non c'è nulla
  da fare. È normale: non è lentezza;
- **Working** = Volumen sta lavorando (stai muovendo qualcosa).

## La prova principale: Sposta 1.000 linee

1. Nel pannello, sotto *Test project*, clicca **Starter house + 1,000 lines**.
   Si apre la casa di esempio con 1.000 linee nella pianta del piano terra.
2. Clicca dentro la **pianta** (la vista 2D), così è quella attiva.
3. Nel pannello, accanto a *Select test lines*, clicca **1000**.
4. Clicca **Record 30 s**. Da qui hai 30 secondi.
   Premi Record **prima** di avviare Sposta: durante Sposta la piccola barra
   delle opzioni del comando può coprire il pulsante.
5. Avvia **Sposta**: l'icona con le quattro frecce nella barra degli
   strumenti, oppure scrivi `M` nella riga di comando e premi Invio.
6. Clicca un punto base nella pianta e **muovi il mouse con continuità**,
   avanti e indietro, senza cliccare, fino alla fine dei 30 secondi.
   Non serve confermare lo spostamento.
7. Alla fine compare *Saved: Volumen-diagnostica-….txt*. Premi **Esc** per
   annullare lo spostamento.
8. Clicca **Open the folder**: si apre la cartella con i rapporti
   (`%APPDATA%\CreaBIM\diagnostics`).

Puoi interrompere prima con **Stop and save**: il rapporto si salva lo
stesso.

## Le altre prove (stessa procedura)

Per ognuna: apri il progetto di prova, seleziona le linee, Record, Sposta,
muovi il mouse per 30 secondi, Esc.

| Prova | Progetto di prova | Linee da selezionare |
|---|---|---|
| A | Starter house | nessuna: seleziona un muro col mouse |
| B | Starter house + 1,000 lines | 1 |
| C | Starter house + 1,000 lines | 1000 (la prova principale) |
| D | Starter house + 10,000 lines | 1 |
| E | Starter house + 10,000 lines | 1000 |
| F | Small house + 10,000 lines | 100 |

La prova E può essere molto lenta: è previsto, è uno dei casi da misurare.

Due prove senza Sposta, sulla Starter house:
- **G, pan e zoom:** Record, poi per 30 secondi sposta la pianta
  tenendo premuta la rotellina e fai zoom con la rotellina;
- **H, 3D:** Record, poi per 30 secondi ruota la vista 3D col mouse.

## Ripetere a una risoluzione più bassa

Il monitor 4K richiede molto lavoro alla scheda grafica integrata. Per
capire quanto pesa:
1. chiudi Volumen;
2. Impostazioni di Windows › Sistema › Schermo › Risoluzione dello schermo:
   scegli **1920 × 1080**;
3. riapri Volumen e ripeti almeno le prove **A**, **C** e **G**;
4. alla fine riporta la risoluzione a 3840 × 2160.

Il rapporto registra da solo la risoluzione e la frequenza dello schermo.
Non serve annotarle.

## Cosa mandare

Tutti i file `Volumen-diagnostica-….txt` della cartella. Scrivi accanto
a ciascuno la lettera della prova (A, B, C…) e la tua impressione: fluido,
un po' in ritardo, molto in ritardo.

## Come leggere il pannello (facoltativo)

| Voce | Significato |
|---|---|
| FPS while working (measured) | Fotogrammi al secondo mentre lavori. 60 = fluido su un monitor a 60 Hz |
| ms between frames (measured) | Tempo tra un fotogramma e il successivo (mediana / p95 / p99 / massimo) |
| ms CPU per frame = internal latency (measured) | Tempo che Volumen impiega a preparare un fotogramma, dalla lettura del mouse alla consegna alla scheda grafica |
| ms waiting for the screen (estimate) | Attesa dello schermo e della coda: **stimata**, non misurata |
| GPU time | Non disponibile: il motore grafico non lo fornisce |
| Model updates with nothing moving | Lavoro fatto col mouse fermo: dovrebbe essere 0 |

**Mediana, p95, p99:** la mediana è il valore tipico. Il p95 è il valore
superato solo dal 5% dei fotogrammi più lenti, il p99 solo dall'1%. Il p95 e
il p99 rivelano gli scatti.

**Latenza interna e latenza fisica non sono la stessa cosa.** La latenza
interna è solo il lavoro di Volumen. Il tempo vero tra il movimento del mouse
e l'immagine sul monitor è più lungo: comprende anche il mouse, Windows, la
coda dei fotogrammi, il monitor. Il programma non può misurarlo: servirebbe
una telecamera ad alta velocità.
