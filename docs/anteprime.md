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

## Come viene sostituita (senza mai cancellarla)

Lo script è `.github/scripts/publish-preview.sh`:

1. se nel frattempo la PR ha un commit più nuovo, non pubblica nulla (lo farà
   la build più nuova);
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
nuovo; e controlla che la release non venga mai cancellata. Gira nella CI a
ogni build. È stato verificato anche che fallisce davvero se lo script perde
il controllo delle dimensioni o torna a ricreare la release.
