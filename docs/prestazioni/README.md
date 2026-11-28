# Prestazioni interattive di Volumen

**Priorità strategica (decisa il 28/11/2026):** fluidità, immediatezza,
precisione e sensazione di controllo del disegno, al livello di AutoCAD.
Le nuove funzionalità non indispensabili restano sospese; le correzioni che
riguardano l'integrità dei progetti mantengono la precedenza.

Versione di riferimento prima della diagnostica: **6f6f94f**.

| File | Cosa contiene |
|---|---|
| [analisi.md](analisi.md) | L'analisi completa del percorso mouse → modello → schermo, con le misure (le righe sbagliate sono segnate) |
| [misure-grezze/](misure-grezze/) | Le uscite dei banchi di prova e i contatori `Perf` raccolti |
| [prove/banco-prova-analisi.rs](prove/banco-prova-analisi.rs) | Il banco usato per l'analisi (storico; quello permanente è in `crates/creabim-app/src/bench_tests.rs`) |
| [prove/modifiche-diagnostiche-solo-copia-isolata.diff](prove/modifiche-diagnostiche-solo-copia-isolata.diff) | Le modifiche fatte **solo in una copia isolata** per isolare le cause; mai nel programma |

## Le conclusioni

### Confermate con misure

- **C-7, la più grave con gruppi di linee:**
  - `Modal::snap_exclude` (`modal.rs`) chiama `Model::dependents`, che
    scorre tutto il modello, per ogni elemento selezionato, a ogni
    fotogramma di Sposta;
  - con 10.000 linee in pianta: 1.000 spostate costano circa 380 ms per
    fotogramma, 10.000 spostate 2,2–2,4 s;
  - calcolando l'esclusione una volta sola (prova isolata) i tempi
    scendono a 180 ms e 370–560 ms;
  - durante il movimento gli snap sono già congelati, quindi l'esclusione
    non cambia mai.
- **C-1:** la pianta viene ricostruita per intero a ogni fotogramma di
  Sposta o di un trascinamento: 13–19 ms (casa di esempio), 120–140 ms con
  10.000 linee. Il costo dipende da tutto il progetto, non dalla selezione.
- **C-2:** durante Sposta la modifica si riapplica anche col mouse fermo.
  Ogni nuova revisione chiede un altro fotogramma: ciclo continuo, CPU
  sempre occupata, coda di presentazione piena.
- **C-3:** le operazioni che non modificano il modello (pan, rotellina,
  elastico, selezione a finestra) costano meno di 1 ms.
- **C-5:** il pannello delle proprietà costa circa 4 ms a ogni fotogramma.
- **C-6:** durante lo zoom l'atlante dei caratteri cresce fino a
  4096×4096 (circa 64 MB) ed è ricaricato per intero sulla GPU.
  Fotogrammi fino a 73 ms a 4K.

### Dal codice, da misurare sul PC dell'utente

- Presentazione con VSync e fino a 2 fotogrammi in coda (predefinito di
  egui-wgpu).
- Vista 3D ridisegnata a ogni fotogramma (4K, multicampionamento 4×,
  ombre, cielo).
- Vertici 3D ricaricati per intero a ogni scena nuova.
- Il cursore di Windows è indipendente dalla geometria disegnata.

## Correzioni (errori del banco di prova)

1. **Selezione sbagliata nel primo banco.** Le linee venivano aggiunte alla
   selezione una alla volta con un'opzione che ogni volta la svuotava: in
   realtà si spostava **1 linea** in una pianta con N linee. I valori
   "Sposta 10 / 100 / 1.000 / 10.000 linee = 16 / 23 / 23 / 142 ms" sono
   quindi sbagliati. Misure corrette: [misure-grezze/3-selezione-contro-progetto.txt](misure-grezze/3-selezione-contro-progetto.txt).
   Da allora ogni banco **verifica la dimensione della selezione** prima di
   misurare.
2. **Variabile d'ambiente vuota.** In una prova isolata il codice
   diagnostico guardava solo se la variabile esisteva, e una variabile
   vuota bastava ad attivarlo. Così entrambe le serie confrontate usavano
   il calcolo ottimizzato. La misura di base è stata ripetuta senza la
   variabile (379 ms e 2.393 ms, coerenti con le misure precedenti).

## Ipotesi escluse con una prova

- **La verifica di selezione lineare (`Selection::contains`) non è la
  causa** del costo che cresce con la selezione. Con una ricerca a costo
  costante (solo copia isolata) i tempi sono rimasti uguali.
- **L'impronta del disegno calcolata a ogni fotogramma (`drawing_print`)
  è trascurabile:** 0,08 ms con 11.600 elementi disegnati.
- **La copia del modello per i lavori in sottofondo è piccola:** 0,5 ms per
  la casa, 2,5 ms con 10.000 linee.
- **La risoluzione 4K pesa poco sulla CPU:** Sposta 16–20 ms anche a 4K, e la
  triangolazione passa da 0,4 a 0,55 ms. Il costo per la GPU resta da
  misurare sul PC dell'utente.

## Prove hardware dell'utente

Monitor portato da 29,97 a 60 Hz e HDR spento: il ritardo resta. AutoCAD,
sulla stessa macchina, è fluido. La causa è nel software.

Macchina di riferimento:
- Windows 11 Pro, Intel Core i7-13620H, 32 GB;
- GPU integrata Intel UHD Graphics;
- monitor BenQ PD2700U, 3840×2160 via HDMI, 60 Hz, HDR spento;
- mouse verticale CSL / WD Plus GmbH 303887.

## Interventi proposti (nessuno ancora autorizzato)

O-10 (esclusione degli snap una volta sola), O-1 (niente lavoro col mouse
fermo), O-2 (coda a 1 fotogramma, come impostazione), O-3 e O-4 (anteprima
separata dal modello in pianta e in 3D), O-9 (3D non ridisegnato se nulla
cambia), O-8 (testo durante lo zoom), O-6 (pannello delle proprietà), O-5
(ricostruzioni parziali). Dettagli e rischi in [analisi.md](analisi.md).

Ogni intervento futuro richiede:
- misure prima e dopo, sulla stessa macchina e con le stesse impostazioni;
- test funzionali e di equivalenza;
- un commit separato;
- la verifica della CI e dell'anteprima.
