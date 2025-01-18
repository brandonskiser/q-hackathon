local M = {}

---@enum Logger.LogLevel
LOG_LEVELS = {
    error = 1,
    warn = 2,
    info = 3,
    debug = 4,
    trace = 5,
}

LEVEL_NAMES = {
    'error',
    'warn',
    'info',
    'debug',
    'trace',
}

M.LOG_LEVELS = LOG_LEVELS

---@param num number
---@param precision number
---@return number
local function to_precision(num, precision)
    if num % 1 == 0 then return num end
    local pow = math.pow(10, precision)
    return math.floor(num * pow) / pow
end

---@param object any
---@return string
local function dstring(object)
    local tp = type(object)

    if tp == "thread"
        or tp == "function"
        or tp == "userdata"
    then
        return string.format("<%s %p>", tp, object)
    elseif tp == "number" then
        return tostring(to_precision(object, 3))
    elseif tp == "table" then
        local mt = getmetatable(object)

        if mt and mt.__tostring then
            return tostring(object)
        elseif vim.islist(object) then
            if #object == 0 then return "[]" end
            local s = ""

            for i = 1, table.maxn(object) do
                if i > 1 then s = s .. ", " end
                s = s .. dstring(object[i])
            end

            return "[ " .. s .. " ]"
        end

        return vim.inspect(object)
    end

    return tostring(object)
end

---Convert an arg list of objects into a table of strings.
---@param ... any
---@return string[]
local function dvalues(...)
    local args = { ... }
    local ret = {}

    for i = 1, select("#", ...) do
        ret[i] = dstring(args[i])
    end

    return ret
end

---@alias Logger.LogFunc fun(self: Logger, ...)
---@alias Logger.FmtLogFunc fun(self: Logger, fmtstr: string, ...)

---@class Logger
---@field error Logger.LogFunc
---@field warn Logger.LogFunc
---@field info Logger.LogFunc
---@field debug Logger.LogFunc
---@field trace Logger.LogFunc
---@field log Logger.LogFunc
---@field name string
---@field log_level Logger.LogLevel # Higher the level, lower the severity.
---@field file_path string          # path to the log file
local Logger = {}

---@param level Logger.LogLevel
---@param ... unknown
function Logger:log(level, ...)
    if level > self.log_level then return end
    local level_name = LEVEL_NAMES[level]
    local val = table.concat(dvalues(...), ' ')
    local msg = string.format('[%s] %s\n', level_name, val)

    local fd, err = vim.uv.fs_open(self.file_path, 'a', tonumber('0644', 8))
    assert(fd, err)
    vim.uv.fs_write(fd, msg)
    vim.uv.fs_close(fd)
end

function Logger:error(...)
    self:log(LOG_LEVELS.error, ...)
end

function Logger:warn(...)
    self:log(LOG_LEVELS.warn, ...)
end

function Logger:info(...)
    self:log(LOG_LEVELS.info, ...)
end

function Logger:debug(...)
    self:log(LOG_LEVELS.debug, ...)
end

function Logger:trace(...)
    self:log(LOG_LEVELS.trace, ...)
end

---@class Opts
---@field name string
---@field log_level? Logger.LogLevel
---@field file_path? string

---@return Logger
function Logger:new(opts)
    local o = {}
    opts = opts or {}
    o.name = opts.name
    o.log_level = opts.log_level or LOG_LEVELS.info
    o.file_path = opts.file_path or string.format("%s/%s.log", vim.fn.stdpath('cache'), self.name)
    setmetatable(o, self)
    self.__index = self
    return o
end

---Creates a new Logger.
---@param opts? Opts
---@return Logger logger
function M.new(opts)
    return Logger:new(opts)
end

return M
