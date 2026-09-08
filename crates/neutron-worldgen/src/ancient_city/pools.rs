// Ancient city template pools extracted from the 26.2 server jar
// (data/minecraft/worldgen/template_pool/ancient_city/*.json). Generated
// by tools/nbt-extract/mc_struct_nbt.py + pool scan -- do not hand-edit.

/// One pool element: weight + content discriminant.
pub(crate) struct PoolElem {
    pub(crate) weight: u32,
    /// `Some(NAME)` = single element (templates.rs const suffix);
    /// `Some("")` = empty element; `None` = list element.
    pub(crate) single: Option<&'static str>,
    pub(crate) list: &'static [&'static str],
    /// feature_pool_element (sculk_patch_ancient_city).
    pub(crate) feature: bool,
}

pub(crate) struct Pool {
    /// fallback pool key ("" = minecraft:empty).
    pub(crate) fallback: &'static str,
    pub(crate) elements: &'static [PoolElem],
}

pub(crate) static POOL_CITY_ENTRANCE: Pool = Pool {
    fallback: "",
    elements: &[
        PoolElem { weight: 1, single: Some("CITY_ENTRANCE_ENTRANCE_CONNECTOR"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_ENTRANCE_ENTRANCE_PATH_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_ENTRANCE_ENTRANCE_PATH_2"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_ENTRANCE_ENTRANCE_PATH_3"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_ENTRANCE_ENTRANCE_PATH_4"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_ENTRANCE_ENTRANCE_PATH_5"), list: &[], feature: false },
    ],
};

pub(crate) static POOL_CITY_CENTER: Pool = Pool {
    fallback: "",
    elements: &[
        PoolElem { weight: 1, single: Some("CITY_CENTER_CITY_CENTER_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_CENTER_CITY_CENTER_2"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_CENTER_CITY_CENTER_3"), list: &[], feature: false },
    ],
};

pub(crate) static POOL_CITY_CENTER_WALLS: Pool = Pool {
    fallback: "",
    elements: &[
        PoolElem { weight: 1, single: Some("CITY_CENTER_WALLS_BOTTOM_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_CENTER_WALLS_BOTTOM_2"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_CENTER_WALLS_BOTTOM_LEFT_CORNER"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_CENTER_WALLS_BOTTOM_RIGHT_CORNER_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_CENTER_WALLS_BOTTOM_RIGHT_CORNER_2"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_CENTER_WALLS_LEFT"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_CENTER_WALLS_RIGHT"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_CENTER_WALLS_TOP"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_CENTER_WALLS_TOP_RIGHT_CORNER"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("CITY_CENTER_WALLS_TOP_LEFT_CORNER"), list: &[], feature: false },
    ],
};

pub(crate) static POOL_SCULK: Pool = Pool {
    fallback: "",
    elements: &[
        PoolElem { weight: 6, single: Some(""), list: &[], feature: true },
        PoolElem { weight: 1, single: Some(""), list: &[], feature: false },
    ],
};

pub(crate) static POOL_STRUCTURES: Pool = Pool {
    fallback: "",
    elements: &[
        PoolElem { weight: 7, single: Some(""), list: &[], feature: false },
        PoolElem { weight: 4, single: Some("STRUCTURES_BARRACKS"), list: &[], feature: false },
        PoolElem { weight: 4, single: Some("STRUCTURES_CHAMBER_1"), list: &[], feature: false },
        PoolElem { weight: 4, single: Some("STRUCTURES_CHAMBER_2"), list: &[], feature: false },
        PoolElem { weight: 4, single: Some("STRUCTURES_CHAMBER_3"), list: &[], feature: false },
        PoolElem { weight: 4, single: Some("STRUCTURES_SAUNA_1"), list: &[], feature: false },
        PoolElem { weight: 4, single: Some("STRUCTURES_SMALL_STATUE"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("STRUCTURES_LARGE_RUIN_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("STRUCTURES_TALL_RUIN_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("STRUCTURES_TALL_RUIN_2"), list: &[], feature: false },
        PoolElem { weight: 2, single: Some("STRUCTURES_TALL_RUIN_3"), list: &[], feature: false },
        PoolElem { weight: 2, single: Some("STRUCTURES_TALL_RUIN_4"), list: &[], feature: false },
        PoolElem { weight: 1, single: None, list: &["STRUCTURES_CAMP_1", "STRUCTURES_CAMP_2", "STRUCTURES_CAMP_3"], feature: false },
        PoolElem { weight: 1, single: Some("STRUCTURES_MEDIUM_RUIN_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("STRUCTURES_MEDIUM_RUIN_2"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("STRUCTURES_SMALL_RUIN_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("STRUCTURES_SMALL_RUIN_2"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("STRUCTURES_LARGE_PILLAR_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("STRUCTURES_MEDIUM_PILLAR_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: None, list: &["STRUCTURES_ICE_BOX_1"], feature: false },
    ],
};

pub(crate) static POOL_WALLS: Pool = Pool {
    fallback: "",
    elements: &[
        PoolElem { weight: 1, single: Some("WALLS_INTACT_CORNER_WALL_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_INTACT_INTERSECTION_WALL_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_INTACT_LSHAPE_WALL_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_INTACT_HORIZONTAL_WALL_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_INTACT_HORIZONTAL_WALL_2"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_INTACT_HORIZONTAL_WALL_STAIRS_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_INTACT_HORIZONTAL_WALL_STAIRS_2"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_INTACT_HORIZONTAL_WALL_STAIRS_3"), list: &[], feature: false },
        PoolElem { weight: 4, single: Some("WALLS_INTACT_HORIZONTAL_WALL_STAIRS_4"), list: &[], feature: false },
        PoolElem { weight: 3, single: Some("WALLS_INTACT_HORIZONTAL_WALL_PASSAGE_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_RUINED_CORNER_WALL_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_RUINED_CORNER_WALL_2"), list: &[], feature: false },
        PoolElem { weight: 2, single: Some("WALLS_RUINED_HORIZONTAL_WALL_STAIRS_1"), list: &[], feature: false },
        PoolElem { weight: 2, single: Some("WALLS_RUINED_HORIZONTAL_WALL_STAIRS_2"), list: &[], feature: false },
        PoolElem { weight: 3, single: Some("WALLS_RUINED_HORIZONTAL_WALL_STAIRS_3"), list: &[], feature: false },
        PoolElem { weight: 3, single: Some("WALLS_RUINED_HORIZONTAL_WALL_STAIRS_4"), list: &[], feature: false },
    ],
};

pub(crate) static POOL_WALLS_NO_CORNERS: Pool = Pool {
    fallback: "",
    elements: &[
        PoolElem { weight: 1, single: Some("WALLS_INTACT_HORIZONTAL_WALL_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_INTACT_HORIZONTAL_WALL_2"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_INTACT_HORIZONTAL_WALL_STAIRS_1"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_INTACT_HORIZONTAL_WALL_STAIRS_2"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_INTACT_HORIZONTAL_WALL_STAIRS_3"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_INTACT_HORIZONTAL_WALL_STAIRS_4"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_INTACT_HORIZONTAL_WALL_STAIRS_5"), list: &[], feature: false },
        PoolElem { weight: 1, single: Some("WALLS_INTACT_HORIZONTAL_WALL_BRIDGE"), list: &[], feature: false },
    ],
};
