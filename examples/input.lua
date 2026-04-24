-- Live reload preserves globals but re-runs the chunk, so locals get
-- fresh nil bindings each save. Keep mutable state in globals (assigned
-- only in _init); keep constants and module aliases local. F5 calls
-- _init to reset.

local input = usagi.input
local gfx = usagi.gfx

function _init()
  state = {
    x = 20,
    y = 20,
    spd = 200,
  }
end

function _update(dt)
  if input.down(input.LEFT) then
    state.x = state.x - state.spd * dt
  end
  if input.down(input.RIGHT) then
    state.x = state.x + state.spd * dt
  end
  if input.down(input.DOWN) then
    state.y = state.y + state.spd * dt
  end
  if input.down(input.UP) then
    state.y = state.y - state.spd * dt
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_WHITE)
  gfx.rect(state.x, state.y, 32, 32, gfx.COLOR_BLACK)
end
