# Copy, paste, delete and undo

## Undo and redo

Every change can be undone: `Ctrl+Z` undoes, `Ctrl+Y` (or `Ctrl+Shift+Z`) redoes. The *Edit* menu says what will be undone ("Undo Move wall"). A drag, a move or a change to a value is one step. The history is saved in the project file, so it is still there after you reopen it.

## Copy, cut and paste

- `Ctrl+C` copies the selection, `Ctrl+X` cuts it, and `Ctrl+V` pastes it back in the same place. Walls keep their doors and windows.
- Then press `M` or `G` to move the pasted copy where it belongs.
- The copy is text, so you can paste it into a second Volumen window.

`Shift+D` (duplicate) copies and starts moving the copy in one go. In Move, `Ctrl` leaves a copy behind at each click.

## Split

**Split** cuts an element in two along a line you choose, where [Divide](divide) cuts it into even parts.

1. Select what to cut: walls, slabs, parts, sketches or masses.
2. Right-click it and pick **Split…** (at the top of the menu), use *Edit › Split…*, or the Split button beside Duplicate.
3. Click a guide, a line or an edge to cut along, or click two points. The drawing snaps work while you pick them. A red see-through cutting plane shows where it cuts, in every view.
4. Click the side to keep selected.

Walls are cut where their line crosses the plane. Slabs, parts drawn from a plan and closed sketches are cut into the outlines on each side (a cut across both arms of a U gives three). An open sketch is cut at every crossing, and a mass by the plane. A split is one undo step; `Esc` leaves.

## Delete

`Del` deletes what is selected. Things that belong to it go too: a wall's doors and windows, an element's tags and attached dimensions. The *Delete* button in *Properties* counts them ("Delete (and 2 attached)"). `Ctrl+Z` brings everything back.

## Hide and show

To get something out of the way for a moment:

- `H` hides the selection, in 3D and the 2D views.
- `Shift+H` shows only the selection.
- `Alt+H` shows everything again.

This is for working only: sheets and exports still show everything, and hidden elements come back when you reopen the project. To leave things out of a view for good, use [visibility](visibility).
