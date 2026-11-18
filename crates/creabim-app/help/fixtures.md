# Electrical, plumbing and HVAC fixtures

Building services fixtures are [objects](furniture) like furniture: you pick them in the Catalogue window and place them with the Object tool. Each one has a 3D shape, the plan symbol services drawings use for it, a usual mounting height and its IFC class. Pipes, ducts and cables between them are not modelled yet.

To see a full set, open the [starter house](starter-house), or *File › New furnished sample house*.

## What there is

| Category | Fixtures | IFC |
|---|---|---|
| Electrical | Double socket, single socket, light switch (1 and 2 gang), consumer unit, smoke detector | IfcOutlet, IfcSwitchingDevice, IfcElectricDistributionBoard, IfcSensor |
| Lighting | Ceiling light (pendant), recessed downlight, wall light | IfcLightFixture |
| Plumbing | Floor drain, combi boiler, outside tap | IfcWasteTerminal, IfcBoiler, IfcValve |
| Heating and ventilation | Radiator, ceiling supply diffuser, ceiling extract grille, split air conditioner, extract fan | IfcSpaceHeater, IfcAirTerminal, IfcUnitaryEquipment, IfcFan |

Basins, WCs, baths and showers are under *Bathroom*, and the kitchen sink under *Kitchen*.

## Placing them

- **Wall fixtures** back onto the nearest wall face within 600 mm, at their usual height above the floor (see the table below). Put an outside tap on the outside face of a wall by clicking just outside it.
- **Ceiling fixtures** (lights, downlights, diffusers, grilles and smoke detectors) are centred where you click and hang from the ceiling over that point. The ceiling is the underside of the floor slab above, or the top of the level (its elevation plus its height) where there is none.
- The **floor drain** sits on the floor where you click.

Wall fixtures go in at these heights, measured to their underside:

| Fixture | Height |
|---|---|
| Sockets | 300 mm |
| Light switches | 1100 mm |
| Consumer unit | 1350 mm |
| Wall light | 1800 mm |
| Radiator | 150 mm |
| Boiler | 1000 mm |
| Outside tap | 600 mm |
| Split air conditioner | 2000 mm |
| Extract fan | 2100 mm |

## Heights

Select a fixture. In *Properties*, **Above its level** sets how high a wall fixture is. For a ceiling fixture the field reads **Below the ceiling**: 0 is flush with the ceiling, and 300 mm drops a pendant by 300 mm. Moving a ceiling fixture up or down, or pushing its top or bottom face, changes the same value. When a slab moves, the ceiling fixtures under it follow.

The type's **Placed** setting has three choices: *Against a wall*, *Free-standing* and *From the ceiling*. **New ones above the floor** (or *below the ceiling*) is the height the Object tool places new ones at. Change it on a socket type to put every new socket at 450 mm, say.

## Plans

Plans draw the standard symbols at a size that reads at 1:50 and 1:100:

- a **socket** is a half circle off the wall with one stroke per socket;
- a **switch** is a small circle with a stroke ending in one tick per gang;
- a **light** is a circle with a cross. A downlight has a second circle inside, and a wall light has a bracket on the wall;
- the **consumer unit** is a box with one half hatched;
- the **smoke detector** is a circle with an S;
- the **radiator** is a hatched strip;
- the **supply diffuser** is a square with its diagonals, and the **extract grille** a square with bars;
- a **fan** is a circle with three blades, a **tap** a valve's bow tie, and a **drain** a grate.

A fixture's symbol is drawn in full even when it is above the cut height. Ceiling lights and diffusers show on the plan below them, as services drawings show them.

## Tags and schedules

- **Tag fixtures** in a plan's header tags every electrical, lighting, plumbing and heating or ventilation fixture on it with its type's **mark**: E01, E02… for electrical types, L for lighting, P for plumbing, M for heating and ventilation, S for sanitary ware, K for kitchen and F for furniture. Furniture, kitchen units and sanitary ware are left out so the plan stays readable; tag those one by one with the **Tag** tool. See [Tags](tags).
- A **Fixtures** schedule counts every fixture by type and level, under a heading for each category, with its mark, mounting height and IFC class. A **Furniture** schedule does the same for furniture. See [Schedules](schedules).

## IFC

Each fixture type exports as its IFC type (IfcOutletType POWEROUTLET, IfcSwitchingDeviceType TOGGLESWITCH, IfcLightFixtureType POINTSOURCE, IfcSpaceHeaterType RADIATOR, IfcAirTerminalType DIFFUSER and so on), with its shape and plan symbol as representation maps. Each placed fixture is an occurrence at its real height. Its type's mounting (*Wall*, *Floor* or *Ceiling*) and default height are kept in the type's Volumen property set, so opening the file brings ceiling fixtures back hung from the ceiling. See [IFC](ifc).
