# PDF, SVG and DXF

## PDF

*File › Sheets as PDF* writes every [sheet](sheets) as a page of one vector PDF, with hatches, dimensions, notes in their real fonts and shaded views. Ready to print or send.

## SVG

*File › This view or sheet as SVG* writes the drawing or sheet in the area you last worked in:

- a sheet, at its paper size;
- a plan, section or elevation, at its scale.

SVG opens in Inkscape, Illustrator, Affinity and web browsers, for presentation work.

## DXF

*File › This view or sheet as DXF* writes the same drawing for CAD programs such as AutoCAD, BricsCAD, LibreCAD and QCAD:

- a plan, section or elevation, full size in millimetres, whatever its scale;
- a sheet, in paper millimetres.

Line weights become layers, so the drawing can be restyled once opened: `HEAVY`, `MEDIUM`, `LIGHT` and `FINE` for solid lines, `DASHED` for dashed ones (with a dashed linetype), `TEXT` for text and `FILL` for solid fills. Coloured lines keep the nearest of the standard CAD colours.

The file is plain DXF R12, which every CAD program reads. Curves are written as short chords, dimensions as their lines and text, hatches as their lines; images and washes of colour are left out.

## DXF and SVG title blocks

*File › Sheet templates… › Import DXF or SVG…* brings in your own title block or sheet frame from a CAD drawing. Any `{name}` in its text becomes a field. See [Sheets and title blocks](sheets#title-blocks-and-masters).
