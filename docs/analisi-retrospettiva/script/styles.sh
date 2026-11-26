#!/bin/bash
A="${ANALISI:?imposta ANALISI alla cartella di lavoro}"
cd $A/styles; mkdir -p out
ALL="0620165 3e54ea8 57a6d13 ac485f0 d92e600 8fc7008 0bc2f49 e9f8376 b364c5a e778fe1 369e1ed 1584697"
for st in HiddenLine Shaded Wireframe White Maquette; do
  case $st in White) V="d92e600 8fc7008 0bc2f49 e9f8376 b364c5a e778fe1 369e1ed 1584697";; Maquette) V="0bc2f49 e9f8376 b364c5a e778fe1 369e1ed 1584697";; *) V=$ALL;; esac
  while read v; do for ver in $V; do echo "$st|$v|$ver"; done; done < $A/cmp/views-sample-layered.txt
done | xargs -P 3 -I{} bash -c 'IFS="|" read st v ver <<< "{}"; f=$(echo "$st-$v" | tr " /" "__"); timeout 300 $A/bin/creabim-$ver view layered-$st.json "$v" out/$f.$ver.svg >/dev/null 2>out/$f.$ver.err; echo -e "$f\t$ver\t$?"' > results.tsv
echo FINE
