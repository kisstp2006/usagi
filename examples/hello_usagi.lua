local MSG = "Hello, Usagi!"
local TEXT_W = 52
local TEXT_H = 8

local x, y, vx, vy

function _init()
  x = 40
  y = 30
  vx = 120
  vy = 60
end

function _update(dt)
  x = x + vx * dt
  y = y + vy * dt

  if x < 0 then
    x = 0
    vx = -vx
  elseif x + TEXT_W > usagi.GAME_W then
    x = usagi.GAME_W - TEXT_W
    vx = -vx
  end

  if y < 0 then
    y = 0
    vy = -vy
  elseif y + TEXT_H > usagi.GAME_H then
    y = usagi.GAME_H - TEXT_H
    vy = -vy
  end
end

function _draw(dt)
  gfx.clear(gfx.COLOR_BLACK)
  gfx.text(MSG, x, y, gfx.COLOR_WHITE)
end
