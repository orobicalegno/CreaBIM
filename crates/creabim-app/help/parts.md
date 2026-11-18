# Extrusions, sweeps and voids

When a part of the building is not a wall, slab, roof or column, model it as an **extrusion** or a **sweep**. Both are ordinary elements: they have a material and a name, they show cut in plans and sections and solid in 3D, and you can change their sizes later in *Properties*. A **void** has the same shape tools but cuts what it meets instead of being solid.

Volumen does not decide what a part is. A chimney, a hearth, a coping, a plinth and a flight of steps are all made the same way. You give each one its name, its material and the IFC class it exports as.

## Extrusions

An **extrusion** is a plan outline pulled straight up. Use it for a chimney breast, a hearth, a pier or a planter.

1. Pick the **Extrusion** tool (*Create › Building elements*).
2. Set its *Base* (how far above the level it starts) and *Height* in the tool options.
3. Draw the outline like a slab: `1` Polygon, `2` Rectangle, `3` Fill room or `4` From sketch.

*Properties* shows its base and height. Below them is its **IFC class**. It is *IfcBuildingElementProxy* unless you pick another, for example *IfcChimney*, *IfcCovering*, *IfcMember* or *IfcStair*.

## Sweeps

A **sweep** carries a section profile along a path.

1. Pick the **Sweep** tool.
2. In the tool options, pick a profile. **The tool's own** are sized right there:
   - **Coping**: over a wall's top, standing out from both faces by its *Overhang*.
   - **Plinth**: along a wall's base, standing out from its exterior face.
   - **Bar**: a plain rectangle, for a kerb or a rail.
   - **Steps**: a flight of steps (its *Going*, *Rise* and number of *Steps*).

   Or pick one from the **profile library**, with a small picture of each: the presets and the project's own. See [Profiles](profiles).
3. Set the profile's sizes in the tool options.
4. Click a wall to run the profile along it. A coping goes along the wall's top and a plinth along its base. Or click the points of a path anywhere, then click the last point again to finish.

A sweep placed on a wall **follows the wall**. When the wall moves, gets longer or changes height, the sweep goes with it. When the wall is deleted, the sweep goes too. In *Properties* you can switch between the wall's top and base, lift the sweep off that edge, and run it past either end of the wall.

**Steps as one element**: pick *Steps* and click two points. The path runs along the nosings, and the flight climbs to the left of the direction you clicked. Use *Mirror profile* in *Properties* to make it climb the other way.

*Properties* can also change the profile's overall width and height, which rescales it.

A sweep made with a library profile has a **Profile** section in *Properties*: change its profile, open it with **Edit in a view**, **Update** it after the profile changed, or **Save as new**. An open profile (a wire) is thickened when swept. While you click a path, the sweep's section shows as you go.

Any sketch (on any plane, open or closed), slab or extrusion can become a profile: **Make a profile** in its *Properties*.

## Voids

A **void** cuts every wall, slab, roof, terrain, hard landscape area and solid extrusion or sweep it meets. Use it for the firebox in a chimney, a hole through a slab, or a niche in a wall.

1. Pick the **Void** tool.
2. Set its *Base* and *Height* in the tool options.
3. Draw its outline like a slab.

The void is worked out from where things are. Move the void and the hole moves with it. Move it clear of an element and that element is no longer cut. In 3D the void shows as a see-through orange outline that you can click. In plans and sections you see only the hole it leaves. To make a solid part into a void, or a void back into a solid, tick or untick **Void** in *Properties*.

## Drawing on a face

In the 3D view, the Extrusion, Void, Sweep, Slab and Wall opening tools draw on the face under the pointer: a wall, a sloped roof, an angled face. The first click starts on that face (or on the workplane, when one is set), and the corners after it stay on the same plane. Snaps work as in a plan, but only on that face.

| Tool | Drawn on a face |
|---|---|
| Extrusion | Stands square out of the face, as high as *Height*. |
| Void | Cuts square into the face, as deep as *Depth on a face*: a roof light, a niche. |
| Sweep | Its path runs on the face, and its profile stands out of it: a band up and across a wall. `Enter` finishes an open path; clicking the first point closes it. |
| Slab | Only on a level face: its top goes on that face. |
| Wall opening | Once the workplane is on a wall face (*Workplane › Pick a face*): the rectangle around what you draw. Without one, a click places an opening as before. |

The extrusion, void or sweep keeps the plane it was drawn on. In *Properties* its *Offset from the face* and *Depth* run along the face's normal. Its corners in Edit mode move on that plane, and moving or turning it takes the plane along.

## Reshaping

An extrusion, a void and a sweep through points have the same grips as a slab. Drag a corner or an edge to reshape it. To add or remove corners, or to change an extrusion's holes, open it in [Edit mode](edit-mode) (`Enter`). A sweep along a wall has no corners of its own: it follows the wall.

## Exchange

In IFC, an extrusion or a sweep is written as the class you picked, with its shape as built. A void is written as an *IfcOpeningElement* in each element it cuts. Those elements keep their uncut shape, so other tools see a proper opening. The IFCX format carries every part's own parameters, so Volumen reads it back as the same editable extrusion, sweep or void. See [IFC](ifc).

Views can hide extrusions and sweeps through the *Extrusions and sweeps* category. Voids are in *Openings, shafts and void cutters* (see [Visibility](visibility)).
