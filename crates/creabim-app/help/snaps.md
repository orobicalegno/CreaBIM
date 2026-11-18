# Snaps

Snaps work as in AutoCAD. While you draw or move, the crosshairs lock onto exact points of the model (AutoSnap's magnet), so walls meet and lines join without guessing. A green marker shows the kind of snap, and a tooltip names it, such as *Endpoint* or *Midpoint*.

## The status bar

The buttons at the bottom work as AutoCAD's:

| Button | Key | What it does |
|---|---|---|
| **GRID** | `F7` | The grid behind drawings |
| **SNAP** | `F9` | Snap mode: points land on a grid of the spacing beside it |
| **ORTHO** | `F8` | From the last point only along X or Y |
| **POLAR** | `F10` | Polar tracking at the increment angle beside it (90°, 45°, 30°…) |
| **OSNAP** | `F3` | Object snaps, with the menu of modes beside it |
| **OTRACK** | `F11` | Object snap tracking: rest a moment on a snap point, then move off along X or Y to track from it |

Ortho and Polar turn each other off, as in AutoCAD.

## Object snap modes

Tick them in the menu beside **OSNAP**, or in *Object snap settings…* (the Drafting settings window). As AutoCAD starts, Endpoint, Center, Intersection and Extension are on.

- **Endpoint** (square): the ends and corners of walls, slabs, lines.
- **Midpoint** (triangle): the middle of an edge.
- **Center** (circle): the centre of circles and arcs.
- **Geometric center** (circle with a star): the centre of a closed shape.
- **Node** (circle with a cross): the even divisions of edges (*Nodes on edges* in the menu: 3, 4, 5, 6, 8 or 10 parts).
- **Quadrant** (diamond): the quarter points of circles and arcs.
- **Intersection** (X): where two edges cross.
- **Extension**: rest on an edge, then move along its line past its end.
- **Insertion** (two squares): where a text is placed from.
- **Perpendicular** (right angle): square to an edge from the last point.
- **Tangent** (circle with a bar): touching a circle or arc from the last point.
- **Nearest** (hourglass): any point on an edge.
- **Apparent intersection** (X in a square): where two lines would cross if they ran on.
- **Parallel**: rest on an edge, then move parallel to it from the last point.

Volumen also snaps to its own objects: grid and level lines, [guide lines](guides), and, if you tick it under *Volumen's own snaps*, in line with the corners of the shape being drawn.

## One snap for the next point

`Shift` + right-click in a plan or section opens the object snap menu: the snap you pick is the only one for the next point, as AutoCAD's override. *None* picks the next point free of snaps. Hold `Alt` while clicking to suspend snapping for that click. When several points are under the pointer, `Tab` steps through them.

## AutoSnap and the crosshairs

*Drafting settings › AutoSnap* sets the marker size, the aperture (how near the pointer must come) and whether the tooltip shows. The crosshairs, the pickbox and their size are in *Settings › Interface › 2D cursor*.

## Snap spacing and distance marks

The menu beside **SNAP** sets the spacing (10, 50 or 100 mm, or any you type). On, points land on its grid, and lengths along polar and tracking lines round to it. *Distance marks*, in the same menu, show faint marks every spacing along a drag, and hold the drag when it comes within 10 px of one. See [Handles and grips](handles).

## Snapping to other levels

A plan's [underlay](levels#underlay) shows another level faintly, and its lines can be snapped to.
