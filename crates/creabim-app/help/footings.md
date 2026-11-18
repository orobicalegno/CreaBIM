# Footings

A **footing** is the foundation under the building. A **strip footing** runs under a wall. A **pad footing** sits under a column or stands on its own.

## Placing footings

1. Pick the **Footing** tool (in *Create › Building elements*, next to Railing).
2. In the tool options pick **Strip** or **Pad**, then set its *Width* (for a pad, its *Size*) and *Thickness*. You can also pick a footing **Type**.
3. Click to place:
   - **Strip:** click a wall to put a strip under it. To draw a strip on its own, click its two ends anywhere else.
   - **Pad:** click a column to put a pad under it, or click anywhere to place one on its own.

Keep clicking walls (or columns) to give each its footing: the tool stays on until you press `Esc`.

To add footings under many elements at once:

- **Drag a box** with the Footing tool, as in Select. Every wall it touches gets a strip (with *Strip* picked), or every column a pad (with *Pad* picked), all in one undo step.
- Or select walls and columns first, then click **Under N selected** in the tool options, or right-click and choose **Footings under selection**. Each wall gets a strip and each column gets a pad.

Anything that already has a footing is skipped, and curtain walls never get one.

## Footings follow what they carry

A footing placed under a wall or column is **hosted** by it:

- A strip runs the wall's length. Where walls are joined, their strips are mitred at corners and trimmed at tees, the same way the walls are. Where a wall ends free, the strip runs past it by half the extra width.
- A pad stays centred under its column and turns with it.
- When the wall or column moves, the footing moves with it. When it is deleted, its footing is deleted too.
- Its depth is measured from the host's base. If the wall or column stands higher on its level, the footing goes up with it.

To make a hosted footing independent, click **Let go of it** in *Properties*. It stays where it is but no longer follows the host.

## Size and depth

*Properties* shows a footing's *Width*, a pad's *Length*, its *Thickness*, and **Top below**: how far its top sits below the base of its wall or column. For a footing on its own, this is measured from its level. The last line gives its top and underside elevations.

A footing on its own also has its *Level* and position, and for a pad, its *Rotation*.

Footing **types** carry a kind (strip or pad), size, thickness and material. They come with four starters: *Strip footing 600 × 300*, *Strip footing 900 × 400*, *Pad footing 1200 × 1200 × 400* and *Pad footing 1800 × 1800 × 500*. See [Types](types).

## Handles

Select a footing to see its handles. They work in plans, sections, elevations and 3D:

- **Width** and, on a pad, **Length**: drag a side.
- **Top** and **Underside**: drag up or down in a section to change the depth or the thickness.
- **Ends** and **Centre**: move a footing that stands on its own. A hosted footing moves with its host instead.

[Edit mode](edit-mode) shows the same handles. [Move](move) and *Rotate* work on footings that stand on their own.

## In drawings and schedules

Footings sit below the floor, so plans show them **dashed**. A plan cut low enough to pass through a footing shows it cut instead. Sections and elevations draw them cut or in view, and 3D shows them in concrete grey.

*File › New schedule › Columns and beams* lists footings too, with their type, level and size. Use the *Footings* category in [Visibility](visibility) to hide them in a view.

## IFC

Footings are exported as **IfcFooting**, with predefined type *STRIP_FOOTING* or *PAD_FOOTING* and their types as **IfcFootingType**. The *Volumen_Footing* property set keeps the size, depth and host, so a footing comes back as a footing, still under its wall or column, when the file is opened again. See [IFC](ifc).
