function _config()
  return { title = "Input Test" }
end

function _init()
  left_down = false
  right_down = false
  up_down = false
  down_down = false
  confirm_down = false
  cancel_down = false
end

function _update(_dt)
  if input.down(input.UP) then
    up_down = true
  else
    up_down = false
  end
  if input.down(input.DOWN) then
    down_down = true
  else
    down_down = false
  end
  if input.down(input.LEFT) then
    left_down = true
  else
    left_down = false
  end
  if input.down(input.RIGHT) then
    right_down = true
  else
    right_down = false
  end
  if input.down(input.CONFIRM) then
    confirm_down = true
  else
    confirm_down = false
  end
  if input.down(input.CANCEL) then
    cancel_down = true
  else
    cancel_down = false
  end
end

function _draw(_dt)
  gfx.clear(gfx.COLOR_BLACK)

  gfx.text("INPUT TEST", 10, 10, gfx.COLOR_WHITE)

  if up_down then
    gfx.spr(2, 60, 40)
  else
    gfx.spr(1, 60, 40)
  end
  gfx.text("UP", 60, 60, gfx.COLOR_WHITE)

  if down_down then
    gfx.spr(2, 60, 80)
  else
    gfx.spr(1, 60, 80)
  end
  gfx.text("DOWN", 60, 100, gfx.COLOR_WHITE)

  if left_down then
    gfx.spr(2, 20, 60)
  else
    gfx.spr(1, 20, 60)
  end
  gfx.text("LEFT", 20, 80, gfx.COLOR_WHITE)

  if right_down then
    gfx.spr(2, 100, 60)
  else
    gfx.spr(1, 100, 60)
  end
  gfx.text("RIGHT", 100, 80, gfx.COLOR_WHITE)

  if confirm_down then
    gfx.spr(2, 180, 40)
  else
    gfx.spr(1, 180, 40)
  end
  gfx.text("CONFIRM", 180, 60, gfx.COLOR_WHITE)

  if cancel_down then
    gfx.spr(2, 180, 80)
  else
    gfx.spr(1, 180, 80)
  end
  gfx.text("CANCEL", 180, 100, gfx.COLOR_WHITE)
end
