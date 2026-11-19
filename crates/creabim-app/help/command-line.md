# Command line and the Disegna menu

Volumen has AutoCAD's command line, over the foot of the drawings. Type a command or its alias in a plan or section, then press `Enter` or `Space`: `L` draws a line, `C` a circle, `TR` trims. The command line shows what the command asks for next, and the last two things it said.

The **Disegna** menu has the same commands, in Italian, each with its alias beside it; picking one runs it on the command line.

## Points and distances

When a command asks for a point, click it in the drawing (with the [snaps](snaps)) or type it:

| Typed | Means |
|---|---|
| `2000,1500` | the point 2000 along X and 1500 along Y from the origin |
| `@3000,0` | 3000 along X from the last point |
| `@2500<45` | 2500 from the last point, at 45° (anticlockwise from X) |
| `1200` | 1200 from the last point, towards the pointer (direct distance entry) |
| `@` | the last point itself |

While a command asks for a point you can also type `FROM` (`DA`): a base point, then the offset from it (`@500,0`); or `M2P` (`PM`): the point halfway between two points.

Lengths are in the project's units (*Units* in the status bar); any unit can be typed, such as `3m` or `10'6"`. With **ORTHO** (`F8`) or **POLAR** (`F10`) on, a distance typed alone follows the direction they hold.

## Keys

- `Enter` or `Space`: end what was typed. With nothing typed, `Enter` ends the shape being drawn, or repeats the last command.
- `Esc`: cancel the command.
- `Up` and `Down`: what was typed before.
- `Backspace`: take back a character.

While the command line is on, typing in a drawing goes to it, as in AutoCAD: single letters are aliases, not shortcuts. `Ctrl` shortcuts (`Ctrl+Z`, `Ctrl+C`…) and the function keys still work. Turn it off in the **Disegna** menu, or in *Settings › Interface › Command line*.

## Commands

The AutoCAD aliases and the Italian names both work, and `_LINE` works as in AutoCAD in Italian.

| Command | Aliases | Does |
|---|---|---|
| LINE | `L`, `LINEA` | Lines through points; `C` closes, `U` takes the last point back |
| PLINE | `PL`, `POLILINEA` | A polyline through points; `A` for arc segments (each tangent to the last), `L` back to straight ones, `C` closes, `U` takes back |
| XLINE | `XL`, `RAY`, `LINEACOSTR` | A construction line ([guide](guides)) |
| CIRCLE | `C`, `CERCHIO` | Centre and radius; `2P` and `3P` by two or three points; `TTR` touching two lines, with its radius |
| ARC | `A`, `ARCO` | By three points; `C` by start, centre and end |
| RECTANG | `REC`, `RETTANGOLO` | Two corners: `@4000,3000` for its size |
| POLYGON | `POL`, `POLIGONO` | The number of sides, the centre, a corner |
| ELLIPSE | `EL`, `ELLISSE` | Centre, an axis end, the other axis |
| SPLINE | `SPL` | A smooth curve through points |
| HATCH | `H`, `TRATTEGGIO` | Click inside closed linework or a room to hatch it; `S` to draw its corners instead |
| DONUT | `DO`, `CIAMBELLA` | Inside and outside diameters, then its centres: filled rings |
| POINT | `PO`, `PUNTO` | Points (small rings whose centre snaps) |
| TEXT | `T`, `MT`, `TESTO` | A note |
| DIMLINEAR | `DLI`, `DIM`, `QUOTA` | A dimension; `DAN`, `DRA`, `DDI` for angles, radii and diameters |
| ERASE | `E`, `CANCELLA` | Delete what is selected |
| MOVE, COPY, ROTATE, MIRROR, SCALE | `M`, `CO`, `RO`, `MI`, `SC` (`SPOSTA`, `COPIA`, `RUOTA`, `SPECCHIO`, `SCALA`) | On what is selected; with nothing selected, select, then `Enter` |
| TRIM, EXTEND | `TR`, `EX` (`TAGLIA`, `ESTENDI`) | Linework |
| FILLET, CHAMFER, OFFSET | `F`, `CHA`, `O` (`RACCORDO`, `CIMA`) | First the radius or distance, then click the lines |
| ARRAY, ARRAYPOLAR | `AR` (`SERIE`) | Rectangular and polar arrays |
| STRETCH | `S`, `STIRA` | A window round the ends to move, a base point, a second point: the ends inside move, the rest stays |
| LENGTHEN | `LEN`, `ALLUNGA` | A length to add (negative takes away), then click lines near the end to change |
| BREAK | `BR`, `SPEZZA` | Click the line at the first break point, then the second: the part between goes |
| BREAKATPOINT | `BRP`, `SPEZZAINPUNTO` | Split a line in two where you click |
| JOIN | `J`, `UNISCI` | Lines that meet end to end, as one |
| MATCHPROP | `MA`, `COPIAPROP` | Click the source line, then the lines to give its pen, layer, line type, colour and fill |
| DIVIDE, MEASURE | `DIV`, `ME` (`DIVIDI`, `MISURA`) | Points along a line: in even parts, or every so far |
| EXPLODE | `X`, `ESPLODI` | A placed symbol back into linework |
| BLOCK | `B`, `BLOCCO` | A symbol from the linework selected |
| UNDO, REDO | `U` | |
| DIST, ID, AREA | `DI`, `ID`, `AA` | Measure a distance, a point's coordinates, an area by its corners (`Enter` totals it) |
| LAYER | `LA` | The [drafting layers](details#layers-in-a-tree) |
| OSNAP | `OS`, `DS` | The Drafting settings: snaps and tracking |
| ORTHO | | Ortho on or off |
| ZOOM | `Z` | `E` everything, `W` a window of two corners, `P` the view before |

Volumen's own elements have commands too: `WALL` (`MURO`), `DOOR` (`PORTA`), `WINDOW` (`FINESTRA`), `SLAB` (`SOLAIO`), `COLUMN` (`PILASTRO`), `BEAM` (`TRAVE`), `ROOM` (`LOCALE`), `SECTION` (`SEZIONE`), `GRID` and `NORTH` (`NORD`). Their points can be typed like any other.
