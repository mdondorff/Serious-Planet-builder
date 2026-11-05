## Decisions
1. **Layout.** The channel block follows the driver block: header 128 B, climate 324 B at 128, driver 324 B at 452, channel 324 B at 776, texels at 1,100. Each word is the fill depth code in bits 0-7, the erosion depth code in bits 8-15, bits 16-31 zero (reserved: a set bit is refused when a slot is read back).
2. **Codes.** `round(255 sqrt(depth / 256 m))`: 4 mm at the first code, 3 cm at 1 m, 2 m at the top. The erosion depth reaches hundreds of metres in a deep valley and the fill depth some tens; one scale serves both. The 0.21 m threshold the explorer named for the deposit (half a code step at 1 m) is a deposit-code matter of 4d; these codes resolve 0.3 m and 0.5 m (SPIKE-100 D1's thresholds) to a few centimetres.
3. **Source.** `ChannelSample` gains `fill_m` and `erosion_m`; the erosion layer's field fills them from `ChannelSource::fill_depth_m` and `erosion_depth_m` (4a: `max(0, deposited - (change + pit))`, sharing the layer's decoded tile). A failed read records like a height's and leaves the texel at 0.
4. **Keys.** `channel_word` folds `CHANNEL_BLOCK_VERSION` and the depth scale: a field with channels has other mesh keys than before; `MESH_VERSION` 11 changes every key and hash (the generators' mesh key, the compact content hashes, the plan snapshots), recorded as known answers with their reasons.
5. **Alignment.** The planner's 9,812 B slot is a multiple of 4 and not of 8 or 16 (1,100 = 4 x 275). The terrain shader reads the slot as `array<u32>` words only and the block is bound whole (`as_entire_binding`); uploads use `write_buffer` at multiples of 4. A test pins: the declaration, no vector or struct view or member access on a block element, no binding offset or dynamic offset.
6. **Default look.** The shader and the look are untouched, so no picture changes; goldens stay.

## Risks
- A future vec4 read of the slot or a dynamic-offset binding breaks the alignment assumption; the pin test fails then and the fix is padding to 9,824 B.
- Memory: +324 B in every resident compact slot (about +3.4 %), +324 B in every prefilled tile of a pack.
