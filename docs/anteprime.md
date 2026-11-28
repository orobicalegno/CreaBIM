# Le anteprime di Volumen

Ogni pull request da un ramo `volumen-*` di questo repository pubblica le sue
build come pre-release **`volumen-anteprima`**, scaricabile senza account:

<https://github.com/orobicalegno/CreaBIM/releases/download/volumen-anteprima/Volumen-windows-x64.zip>

## Quando l'anteprima cambia

Solo quando **tutti** i controlli della CI sono passati sul commit:

- `test` su Linux e su Windows (formattazione, clippy, tutti i test, il test
  dello script di pubblicazione);
- `ifc` (export IFC validato con IfcOpenShell, PDF, schedule, formato JSON);
- `windows-build` e `linux-build`.

Se uno fallisce, il passo di pubblicazione non parte e resta l'ultima
anteprima valida. Lo stesso vale se fallisce la pubblicazione stessa.

## Le anteprime precedenti

Ogni anteprima pubblicata ha anche una **copia permanente**, che non viene mai
sostituita né cancellata: una pre-release chiamata

    volumen-anteprima-AAAAMMGG-HHMM-<commit>

(per esempio `volumen-anteprima-20261123-1915-eefdee0`), con gli stessi file.
Si trovano nella pagina *Releases* del repository. Servono a riprendere una
versione precedente, o a capire da quando c'è un problema scaricando la
versione di qualche giorno prima.

Per ora non vengono eliminate in automatico: una politica di conservazione
verrà decisa più avanti.

## Come viene sostituita (senza mai cancellarla)

Lo script è `.github/scripts/publish-preview.sh`:

1. se nel frattempo la PR ha un commit più nuovo, non pubblica nulla (lo farà
   la build più nuova);
1. fa la copia permanente: prima come bozza invisibile, poi controlla che
   ci siano tutti i file con la dimensione giusta, e solo allora la rende
   pubblica. Se qualcosa non va elimina quella bozza e si ferma: l'anteprima
   non viene toccata. Una nuova esecuzione per lo stesso commit riusa la
   copia già fatta;
2. rimette a posto ciò che un'esecuzione interrotta avesse lasciato a metà;
3. carica i file nuovi **accanto** ai vecchi, con il prefisso `next-`, e
   controlla che ci siano tutti e con la dimensione giusta: se no si ferma, e
   l'anteprima resta com'era;
4. solo allora scambia i file uno a uno (il vecchio diventa `prev-`, il nuovo
   prende il nome definitivo; `SHA256SUMS.txt` per ultimo), sposta l'etichetta
   `volumen-anteprima` sul commit e aggiorna le note;
5. infine elimina i file vecchi.

Due pubblicazioni non girano mai insieme (`concurrency: volumen-preview`,
senza interrompere quella in corso).

## Come è verificato

`.github/scripts/test-publish-preview.sh` prova lo script contro una
simulazione di GitHub (`.github/scripts/test/gh`), senza toccare le release
vere: prima pubblicazione, aggiornamento, caricamento interrotto, caricamento
corrotto, interruzione a metà dello scambio, build superata da un commit più
nuovo, copia permanente interrotta o corrotta; e controlla che la release non
venga mai cancellata e che ogni build pubblicata abbia la sua copia. Gira nella CI a
ogni build. È stato verificato anche che fallisce davvero se lo script perde
il controllo delle dimensioni o torna a ricreare la release.
