use {
    super::{
        PixelPriority, Ppu, BG_PAL, BG_TO_OAM_PR, BG_X_FLIP, BG_Y_FLIP, LARGE_SPRITES,
        OBJECTS_ENABLED,
    },
    crate::{
        video::sprites::{SpriteAttr, SPR_BG_WIN_OVER_OBJ, SPR_FLIP_X, SPR_FLIP_Y, SPR_PAL},
        FunctionMode, SCREEN_WIDTH,
    },
    arrayvec::ArrayVec,
    core::cmp::Ordering,
};

impl Ppu {
    pub(crate) fn draw_scanline(&mut self, function_mode: FunctionMode) {
        let mut bg_priority = [PixelPriority::Normal; SCREEN_WIDTH as usize];

        self.draw_background(function_mode, &mut bg_priority);
        self.draw_window(function_mode, &mut bg_priority);
        self.draw_sprites(function_mode, &mut bg_priority);
    }

    fn draw_background(
        &mut self,
        function_mode: FunctionMode,
        bg_priority: &mut [PixelPriority; SCREEN_WIDTH as usize],
    ) {
        let ly = self.ly;
        let scy = self.scy;
        let scx = self.scx;
        let lcdc = self.lcdc;
        let bgp = self.bgp;
        let index_start = SCREEN_WIDTH as usize * ly as usize;

        if lcdc.bg_enabled(function_mode) {
            let tile_map_addr = lcdc.bg_tile_map_addr();
            let y = ly.wrapping_add(scy);
            let row = (y / 8) as u16 * 32;
            let line = ((y % 8) * 2) as u16;

            for i in 0..SCREEN_WIDTH {
                let x = i.wrapping_add(scx);
                let col = (x / 8) as u16;

                let tile_num_addr = tile_map_addr + row + col;
                let tile_number = self.vram.tile_number(tile_num_addr);

                let gb_attr = match function_mode {
                    FunctionMode::Monochrome | FunctionMode::Compatibility => 0,
                    FunctionMode::Color => self.vram.background_attributes(tile_num_addr),
                };

                let tile_data_addr = if gb_attr & BG_Y_FLIP != 0 {
                    lcdc.tile_data_addr(tile_number) + 14 - line
                } else {
                    lcdc.tile_data_addr(tile_number) + line
                };

                let (data_low, data_high) = self.vram.tile_data(tile_data_addr, gb_attr);

                let color_bit = 1
                    << if gb_attr & BG_X_FLIP != 0 {
                        x & 7
                    } else {
                        7 - (x & 7)
                    };

                let color_number =
                    (((data_high & color_bit != 0) as u8) << 1) | (data_low & color_bit != 0) as u8;

                let color = match function_mode {
                    FunctionMode::Monochrome => self
                        .monochrome_palette_colors
                        .get_color(bgp.shade_index(color_number)),
                    FunctionMode::Compatibility => self
                        .cgb_bg_palette
                        .get_color(gb_attr & BG_PAL, bgp.shade_index(color_number)),
                    FunctionMode::Color => self
                        .cgb_bg_palette
                        .get_color(gb_attr & BG_PAL, color_number),
                };

                self.pixel_data
                    .set_pixel_color(index_start + i as usize, color);

                bg_priority[i as usize] = if color_number == 0 {
                    PixelPriority::SpritesOnTop
                } else if gb_attr & BG_TO_OAM_PR != 0 {
                    PixelPriority::BackgroundOnTop
                } else {
                    PixelPriority::Normal
                };
            }
        }
    }

    fn draw_window(
        &mut self,
        function_mode: FunctionMode,
        bg_priority: &mut [PixelPriority; SCREEN_WIDTH as usize],
    ) {
        let ly = self.ly;
        let lcdc = self.lcdc;
        let bgp = self.bgp;
        let index_start = SCREEN_WIDTH as usize * ly as usize;

        let wy = self.wy;

        if lcdc.win_enabled(function_mode) && wy <= ly {
            let tile_map_addr = lcdc.window_tile_map_addr();
            let wx = self.wx.saturating_sub(7);
            let y = ((ly - wy) as u16).wrapping_sub(self.window_lines_skipped) as u8;
            let row = (y / 8) as u16 * 32;
            let line = ((y % 8) * 2) as u16;

            for i in wx..SCREEN_WIDTH {
                self.frame_used_window = true;
                self.scanline_used_window = true;

                let x = i.wrapping_sub(wx);
                let col = (x / 8) as u16;

                let tile_num_addr = tile_map_addr + row + col;
                let tile_number = self.vram.tile_number(tile_num_addr);

                let bg_attr = match function_mode {
                    FunctionMode::Monochrome | FunctionMode::Compatibility => 0,
                    FunctionMode::Color => self.vram.background_attributes(tile_num_addr),
                };

                let tile_data_addr = if bg_attr & BG_Y_FLIP != 0 {
                    lcdc.tile_data_addr(tile_number) + 14 - line
                } else {
                    lcdc.tile_data_addr(tile_number) + line
                };

                let (data_low, data_high) = self.vram.tile_data(tile_data_addr, bg_attr);

                let color_bit = 1
                    << if bg_attr & BG_X_FLIP != 0 {
                        x % 8
                    } else {
                        7 - (x % 8)
                    };

                let color_number =
                    (((data_high & color_bit != 0) as u8) << 1) | (data_low & color_bit != 0) as u8;

                let color = match function_mode {
                    FunctionMode::Monochrome => self
                        .monochrome_palette_colors
                        .get_color(bgp.shade_index(color_number)),
                    FunctionMode::Compatibility => self
                        .cgb_bg_palette
                        .get_color(bg_attr & BG_PAL, bgp.shade_index(color_number)),
                    FunctionMode::Color => self
                        .cgb_bg_palette
                        .get_color(bg_attr & BG_PAL, color_number),
                };

                bg_priority[i as usize] = if color_number == 0 {
                    PixelPriority::SpritesOnTop
                } else if bg_attr & BG_TO_OAM_PR != 0 {
                    PixelPriority::BackgroundOnTop
                } else {
                    PixelPriority::Normal
                };

                self.pixel_data
                    .set_pixel_color(index_start + i as usize, color);
            }
        }

        if self.frame_used_window && !self.scanline_used_window {
            self.window_lines_skipped += 1;
        }
    }

    fn draw_sprites(
        &mut self,
        function_mode: FunctionMode,
        bg_priority: &mut [PixelPriority; SCREEN_WIDTH as usize],
    ) {
        let ly = self.ly;
        let lcdc = self.lcdc;
        let index_start = SCREEN_WIDTH as usize * ly as usize;

        if lcdc.val & OBJECTS_ENABLED != 0 {
            let large_sprites = lcdc.val & LARGE_SPRITES != 0;
            let sprite_height = if large_sprites { 16 } else { 8 };

            let mut spr_ly: ArrayVec<(usize, SpriteAttr), 10> = self
                .oam
                .sprite_attributes_iterator()
                .filter(|sprite| ly.wrapping_sub(sprite.y()) < sprite_height)
                .take(10)
                .enumerate()
                .collect();

            if self.opri & 1 == 0 {
                spr_ly.sort_unstable_by(|(a_pos, a), (b_pos, b)| match b_pos.cmp(a_pos) {
                    Ordering::Equal => a.x().cmp(&b.x()),
                    x => x,
                });
            } else {
                spr_ly.sort_unstable_by(|(a_pos, a), (b_pos, b)| match b.x().cmp(&a.x()) {
                    Ordering::Equal => b_pos.cmp(a_pos),
                    x => x,
                });
            }

            for (_, sprite) in spr_ly {
                let tile_number = if large_sprites {
                    sprite.tile_index() & !1
                } else {
                    sprite.tile_index()
                };

                let tile_data_addr =
                    (tile_number as u16 * 16).wrapping_add(if sprite.flags() & SPR_FLIP_Y != 0 {
                        (sprite_height as u16 - 1)
                            .wrapping_sub((ly.wrapping_sub(sprite.y())) as u16)
                            * 2
                    } else {
                        ly.wrapping_sub(sprite.y()) as u16 * 2
                    });

                let (data_low, data_high) = self.vram.sprite_data(tile_data_addr, &sprite);

                for xi in (0..8).rev() {
                    let target_x = sprite.x().wrapping_add(7 - xi);

                    if target_x >= SCREEN_WIDTH {
                        continue;
                    }

                    if bg_priority[target_x as usize] == PixelPriority::BackgroundOnTop
                        && !self.lcdc.cgb_sprite_master_priority_on(function_mode)
                    {
                        continue;
                    }

                    let color_bit = 1
                        << if sprite.flags() & SPR_FLIP_X != 0 {
                            7 - xi
                        } else {
                            xi
                        };

                    let color_number = (((data_high & color_bit != 0) as u8) << 1)
                        | (data_low & color_bit != 0) as u8;

                    // transparent
                    if color_number == 0 {
                        continue;
                    }

                    let color = match function_mode {
                        FunctionMode::Monochrome => {
                            let palette = if sprite.flags() & SPR_PAL != 0 {
                                self.obp1
                            } else {
                                self.obp0
                            };
                            self.monochrome_palette_colors
                                .get_color(palette.shade_index(color_number))
                        }
                        FunctionMode::Compatibility => {
                            let palette = if sprite.flags() & SPR_PAL != 0 {
                                self.obp1
                            } else {
                                self.obp0
                            };
                            self.cgb_sprite_palette
                                .get_color(0, palette.shade_index(color_number))
                        }
                        FunctionMode::Color => {
                            let cgb_palette = sprite.cgb_palette();
                            self.cgb_sprite_palette.get_color(cgb_palette, color_number)
                        }
                    };

                    if !self.lcdc.cgb_sprite_master_priority_on(function_mode)
                        && sprite.flags() & SPR_BG_WIN_OVER_OBJ != 0
                        && bg_priority[target_x as usize] == PixelPriority::Normal
                    {
                        continue;
                    }

                    self.pixel_data
                        .set_pixel_color(index_start + target_x as usize, color);
                }
            }
        }
    }
}
