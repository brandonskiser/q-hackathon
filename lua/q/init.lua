local M = {}

local state = {}

local cmd_name = "hackathon"
local cmd_path = "/Volumes/workplace/hackathon/target/debug/" .. cmd_name

local function debug(...)
    print(...)
end

local function run_cmd()
end

---Initialize q-ai
---@param opts? table
function M.setup(opts)
    opts = opts or {}
    debug("Loading Q")

    vim.api.nvim_create_user_command(
        'QChat',
        function(tab)
            local prompt = tab.args
            -- debug(tab.args)
            -- debug(vim.inspect(tab.fargs))
            -- debug(tab.nargs)
            if prompt == '' then
                debug("No prompt provided, returning")
                return
            end

            local curr_buf = vim.api.nvim_buf_get_lines(0, 0, -1, false)
            local cmd_result = vim.system({ cmd_path, prompt },
                {
                    text = true,
                    stdin = curr_buf,
                    env = { RUST_LOG = 'debug' }
                }):wait()
            if cmd_result.code ~= 0 then
                debug('command failed with code: ' .. cmd_result.code)
                return
            end

            debug('cmd stdout: ' .. cmd_result.stdout)

            -- Create a new window and display the command output
            local new_buf = vim.api.nvim_create_buf(false, true)
            vim.api.nvim_buf_set_lines(new_buf, 0, -1, false, vim.split(cmd_result.stdout, "\n"))
            local new_win = vim.api.nvim_open_win(new_buf, true, {
                relative = 'editor',
                width = 80,
                height = 20,
                row = 5,
                col = 5,
            })
        end,
        {
            desc = 'Chat with Q',
            nargs = '*'
        }
    )
end

M.setup()

return M
