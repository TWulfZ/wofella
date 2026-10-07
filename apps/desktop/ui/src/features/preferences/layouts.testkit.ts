import type { ColumnDto, FingerDto, HandDto, HandLayoutDto } from "@/ipc/bindings";

const col = (hand: HandDto, finger: FingerDto): ColumnDto => ({ hand, finger });

// Mirrors crates/chart/src/layout.rs PRESETS, profile default first as settings_hand_layouts orders them.
export const K7_LAYOUTS: HandLayoutDto[] = [
  {
    id: "k7.313_right_thumb",
    columns: [
      col("left", "ring"),
      col("left", "middle"),
      col("left", "index"),
      col("right", "thumb"),
      col("right", "index"),
      col("right", "middle"),
      col("right", "ring"),
    ],
  },
  {
    id: "k7.313_left_thumb",
    columns: [
      col("left", "ring"),
      col("left", "middle"),
      col("left", "index"),
      col("left", "thumb"),
      col("right", "index"),
      col("right", "middle"),
      col("right", "ring"),
    ],
  },
  {
    id: "k7.43",
    columns: [
      col("left", "pinky"),
      col("left", "ring"),
      col("left", "middle"),
      col("left", "index"),
      col("right", "index"),
      col("right", "middle"),
      col("right", "ring"),
    ],
  },
  {
    id: "k7.34",
    columns: [
      col("left", "ring"),
      col("left", "middle"),
      col("left", "index"),
      col("right", "index"),
      col("right", "middle"),
      col("right", "ring"),
      col("right", "pinky"),
    ],
  },
  {
    id: "k7.both_thumbs",
    columns: [
      col("left", "ring"),
      col("left", "middle"),
      col("left", "index"),
      col("both", "thumb"),
      col("right", "index"),
      col("right", "middle"),
      col("right", "ring"),
    ],
  },
];
