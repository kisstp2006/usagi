local input = usagi.input
local gfx = usagi.gfx
local state = {}

-- runs once at the game start, useful for loading assets, etc.
function _init()
  state = {
    x = 20,
    y = 20,
    spd = 200,
  }
end

-- runs once every frame (60 FPS)
function _update(dt)
  if input.pressed(input.LEFT) then
    state.x = state.x - state.spd * dt
  end
  if input.pressed(input.RIGHT) then
    state.x = state.x + state.spd * dt
  end
  if input.pressed(input.DOWN) then
    state.y = state.y + state.spd * dt
  end
  if input.pressed(input.UP) then
    state.y = state.y - state.spd * dt
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_WHITE)
  gfx.rect(state.x, state.y, 32, 32, gfx.COLOR_BLACK)
end
