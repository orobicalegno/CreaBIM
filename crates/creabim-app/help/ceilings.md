# Ceilings

A **ceiling** is a thin layer hung under the floor or roof above: plasterboard, or a suspended tile ceiling. It shows in sections and in 3D, has a **reflected ceiling plan** of its own, gives the rooms under it their height, and is what ceiling lights hang from and partitions can stop at.

## Placing ceilings

1. Pick the **Ceiling** tool (in *Create › Building elements*, after Room separator).
2. Click inside a room. The ceiling goes over it and **follows the room**: when the walls move, the ceiling's outline changes with them.

Clicking inside walls where there is no room yet gives a ceiling with the outline of the space there, which doesn't follow the walls. Place the room first for one that does.

Like rooms and slabs, the tool can also draw the ceiling's outline: **Polygon (1)**, **Rectangle (2)** or **From sketch (4)**. Use a drawn ceiling for a bulkhead, or a ceiling over only part of a room.

## Height, thickness and grid

*Properties* has the ceiling's:

- **Height**: of its underside above its level. New ceilings hang at 2500, or lower in a storey too low for that.
- **Thickness**: upwards from the underside; 12.5 for plasterboard.
- **Suspended grid**: ticked for a tile ceiling, with its **Tile** size (600 by default). The grid is drawn in ceiling plans.
- **Material**, like any other element's. See [Materials](materials).

The room under a ceiling takes its height from it, so the room schedule and the IfcSpace's height follow the ceiling.

## Reflected ceiling plans

Tick **Reflected ceiling plan** in a plan's *Properties* (or **Ceiling plan** in its view options). The plan then shows the ceilings, with their grids, and what hangs from them (lights and other ceiling fixtures), with the walls cut as usual. Furniture, floors, stairs and door swings are left out. Duplicate a floor plan first to keep both.

## Walls that stop at the ceiling

A partition can run up to the underside of a ceiling instead of a level or the roof. Select the wall and pick the ceiling in its *Top* options, the same way a wall attaches to a slab or a roof (see [Walls](walls)). The wall then follows the ceiling when its height changes. In the starter house the first-floor partitions stop at the bedroom and bathroom ceilings.

## Fixtures that hang from it

A ceiling light, a smoke detector or an extract fan placed with the Object tool hangs from the ceiling over the spot you click, when there is one. Without one it hangs from the slab or roof above, or at the storey height. See [Electrical, plumbing and HVAC fixtures](fixtures).

## In drawings and IFC

Sections cut ceilings as thin lines at their height; 3D draws them in off-white. Use the *Ceilings* category in [Visibility](visibility) to hide them.

Ceilings are exported as **IfcCovering** with predefined type *CEILING*, linked to the space under them by **IfcRelCoversSpaces**. The *Volumen_Ceiling* property set keeps the grid and the room, so the ceiling comes back over its room when the file is opened again. See [IFC](ifc).

**See also:** [Rooms](rooms)
