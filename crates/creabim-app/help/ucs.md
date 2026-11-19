# UCS and turning the plan

Volumen has AutoCAD's **user coordinate system** in plans: an origin and an X axis of your choice, for drawing square to a building that does not sit square to the world. With a UCS:

- points typed on the [command line](command-line) are read in it: `1000,0` is 1000 along the UCS's X, `@500<90` goes along its Y, and `*1000,0` types in the world's coordinates;
- **ORTHO**, **POLAR** and **OTRACK** ([snaps](snaps)) follow its axes, and so do the snap grid, the grid behind drawings and the crosshairs;
- **RECTANG** draws square to it;
- **ID** and **DIST** give coordinates and angles in it.

The UCS icon shows its X and Y. It sits at the UCS's origin when that is in view, else in the lower left corner, with a little square when the UCS is the world's.

## Setting the UCS

Type `UCS` (or *Disegna › UCS*):

| Answer | Gives |
|---|---|
| a point, then a point on its X axis (or Enter) | the UCS's origin, then the direction of its X axis |
| `W` | the world's coordinate system (WCS) |
| `P` | the UCS before |
| `OB` | along a line or wall edge you click: its origin at the nearer end |
| `Z` | turned about Z by an angle typed |
| `V` | square to the plan as it is turned |
| `S`, `R`, `D` | save the UCS by a name, restore one, delete one; `?` lists them |

Saved UCSs are kept in the project. `UCSMAN` (`UC`) opens the UCS window: the World, the previous one and the saved ones, to set one current, save the current one or delete one.

## Turning the plan: PLAN

`PLAN` turns the plan view so the current UCS's X axis reads level on screen, as AutoCAD's PLAN does. `PLAN W` turns it back to the world's, and `PLAN` then a saved UCS's name turns it to that one. Text stays readable. Panning and zooming work as usual in a turned plan. The plan turns on screen only: sheets and exports print it square to the world.

`UCSICON` takes `ON`, `OFF`, `OR` (at the origin) and `N` (in the corner).
