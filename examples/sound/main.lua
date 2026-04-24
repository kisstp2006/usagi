function _update(dt)
  if input.pressed(input.CONFIRM) then
    sfx.play("jump")
  end
  if input.pressed(input.CANCEL) then
    sfx.play("explosion")
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text("Press CONFIRM for jump.wav", 20, 20, gfx.COLOR_WHITE)
  gfx.text("Press CANCEL for explosion.wav", 20, 40, gfx.COLOR_WHITE)
end
