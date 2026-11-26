#!/bin/bash
A="${ANALISI:?imposta ANALISI alla cartella di lavoro}"
cd $A/cmp; mkdir -p mx
V="0620165 3e54ea8 57a6d13 ac485f0 d92e600 8fc7008 0bc2f49 e9f8376 b364c5a e778fe1 369e1ed 1584697"
job() { s=$1; v="$2"; ver=$3; f=$(echo "$s-$v" | tr ' /' '__'); start=$(date +%s.%N); timeout 300 $A/bin/creabim-$ver view old-$s.json "$v" mx/$f.$ver.svg >/dev/null 2>mx/$f.$ver.err; rc=$?; end=$(date +%s.%N); h=$(sha256sum mx/$f.$ver.svg 2>/dev/null | cut -c1-12); echo -e "$f\t$ver\t$rc\t$h\t$(echo "$end-$start" | bc)"; }
export -f job; export A
for s in sample sample-layered sample-typed sample-furnished sample-garden; do while read v; do for ver in $V; do echo "$s|$v|$ver"; done; done < views-$s.txt; done | xargs -P 3 -I{} bash -c 'IFS="|" read s v ver <<< "{}"; job "$s" "$v" "$ver"' > mx-results.tsv
for s in sample sample-layered sample-typed sample-furnished sample-garden; do for ver in $V; do timeout 300 $A/bin/creabim-$ver ifc old-$s.json mx/$s.$ver.ifc >/dev/null 2>&1; echo -e "IFC-$s\t$ver\t$?\t$(sha256sum mx/$s.$ver.ifc 2>/dev/null|cut -c1-12)"; timeout 600 $A/bin/creabim-$ver pdf old-$s.json mx/$s.$ver.pdf >/dev/null 2>&1; echo -e "PDF-$s\t$ver\t$?\t$(sha256sum mx/$s.$ver.pdf 2>/dev/null|cut -c1-12)"; done; done > mx-exports.tsv
echo FINE
