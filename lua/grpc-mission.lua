-- Allow manually setting GRPC and path settings.
if not _G.GRPC then
  _G.GRPC = {}
end
if not GRPC.luaPath then
  GRPC.luaPath = lfs.writedir() .. [[Scripts\DCS-gRPC\]]
end
if not GRPC.dllPath then
  GRPC.dllPath = lfs.writedir() .. [[Mods\tech\DCS-gRPC\]]
end

-- Let DCS know where to find the DLLs
package.cpath = package.cpath .. GRPC.dllPath .. [[?.dll;]]

-- Make paths available to gRPC hook
local file, err = io.open(lfs.writedir() .. [[Data\dcs-grpc.lua]], "w")
if err then
	env.error("[GRPC] Error writing config")
else
	file:write("luaPath = [[" .. GRPC.luaPath .. "]]\ndllPath = [[" .. GRPC.dllPath .. "]]\n")
	file:flush()
	file:close()
end

-- Load DLL before `require` gets sanitized.
local ok, grpc = pcall(require, "dcs_grpc_hot_reload")
if ok then
  env.info("[GRPC] loaded hot reload version")
else
  grpc = require("dcs_grpc")
end

-- Keep a reference to `lfs` before it gets sanitized
local lfs = _G.lfs

function GRPC.load()
  local env = setmetatable({grpc = grpc, lfs = lfs}, {__index = _G})
  local f = setfenv(assert(loadfile(GRPC.luaPath .. [[grpc.lua]])), env)
  f()
end
