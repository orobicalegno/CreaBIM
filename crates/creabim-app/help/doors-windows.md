# Doors and windows

Doors and windows are hosted by a wall: they cut their opening in it and move with it.

## Placing them

Pick the **Door** or **Window** tool and click on a wall, in a plan, a section, an elevation or 3D. Pick a **Type** in the tool options to place a door or window of that type, or type its **Width**, **Height** and **Sill** there before you click (typing a size drops the type, as a wall's thickness does). The same settings are in the **Next element** tab of *Properties*.

Before you click, a **ghost** shows exactly what lands: the frame and the cut through the whole wall, in plans and in 3D. Off a wall, it says *Move onto a wall*. A door or window stays inside its wall's clear length: it stops at a wall joined at the corner instead of cutting into it.

## Changing them

Select one, then:

- set its *Width*, *Height* and *Sill* (the height of its bottom edge above the wall's base) in *Properties*;
- drag it along its wall to move it (it stays in the wall);
- drag its jamb handles to widen it, or its head and sill handles to change its height, in a plan or in 3D;
- pick a material, and its frame: *Frame width* and *Frame depth*, and for windows *Panes across* and *Panes up*.

## Where the frame stands in the wall

By default a door's or window's frame stands in the middle of the wall's thickness. **Frame at** in its *Properties* puts it against a face of any of the wall's layers instead. The list has every layer from the exterior in, each with its two faces:

- a layer's **exterior face**: the frame's outer face is flush with it, and the frame runs inwards from there. The block's exterior face stands the window against the masonry, behind the insulation; the first layer's exterior face puts it flush with the outside of the wall.
- a layer's **interior face**: the frame's inner face is flush with it, and the frame runs outwards from there. The insulation's interior face stands the window in the insulation; the last layer's interior face puts it flush with the inside of the wall.

**Frame offset** then moves it a set distance towards the interior, or towards the exterior with a minus sign. The frame is kept inside the wall. A wall of one material counts as one layer, so its two faces are the outside and the inside of the wall.

The frame, glass, leaf and swing move together in plans, sections, 3D and IFC, and a window's outside sill still reaches the face of the wall. Where the wall's layers [wrap](assemblies#doors-and-windows-in-layered-walls) into the reveals, they run to the frame wherever it stands. The setting is each door's or window's own; a type doesn't hold it yet.

## Flip, hand and swing

A door has a **hand** (which jamb the hinges are on) and a **swing** (which side it opens to). *Properties* reads, for example, *Hung right, in*.

- `Space`, or *Flip › Swing* in *Properties*, flips the swing.
- `Shift+Space`, or *Flip › Hand*, flips the hand. *Hung* reads the result, such as *left, in*.
- A window flips inside and outside with `Space`, or *Flip inside / outside* in *Properties*.

![A selected door, with Hung and its Hand and Swing flips in Properties](img/door-flip.jpg)

## Tags and schedules

The **Tag** tool labels a door or window with its mark (D01, W01). *Tag all* (`Shift+T`), or *Tag openings* in the view's header, tags all of them in the view at once. A door or window schedule lists them all, and you can edit sizes in the table. See [Tags](tags) and [Schedules](schedules).

## Holes with nothing in them

For a bare hole in a wall, use the **Wall opening** tool. See [Holes, wall openings and shafts](holes).
