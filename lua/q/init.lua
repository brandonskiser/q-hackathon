local M = {}

local state = {}

local MOCK = true
local TEST_FAILURE = false

local cmd_name = "hackathon"
local cmd_path = "/Volumes/workplace/hackathon/target/debug/" .. cmd_name

local function debug(...)
    print(...)
end

local mock_res = [[
{
    "type": "code",
    "message": [
        {
            "language": "lua",
            "code": "local M = {}\\n\\nfunction M.add(a, b)\\n    return a + b\\nend\\n\\nreturn M\\n"
        }
    ]
}
]]

local mock_code_invalid_json = [[
{
    "type": "code",
    "message": [
        {
            "language": "lua",
            "code": "local M = {}\\n\\nfunction M.add(a, b)\\n    return a + b\\nend\\n\\nreturn M\\n
        }
    ]
}
]]

---@class CallOpts
---@field prompt string
---@field stdin? string | string[]

---@param opts CallOpts
local function call(opts)
    if MOCK then
        return {
            code = 0,
            signal = 0,
            stdout = mock_res
        }
    end
    local cmd_result = vim.system({ cmd_path, opts.prompt },
        {
            text = true,
            stdin = opts.stdin,
            env = { RUST_LOG = 'debug' }
        }):wait()
        print('hi')
    return cmd_result

end


---Initialize q-ai
---@param opts? table
function M.setup(opts)
    opts = opts or {}

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
            local cmd_result = call({ prompt = prompt, stdin = curr_buf })
            if cmd_result.code ~= 0 then
                debug('command failed with code: ' .. cmd_result.code)
                return
            end


            local out = vim.json.decode(cmd_result.stdout)
            -- debug('cmd json: ', vim.inspect(out))
            if out.type == 'chat' then
                debug('Received chat response')
            else
                debug('Received code response')
                -- create a new buffer to display the results
                local new_buf = vim.api.nvim_create_buf(false, true)
                vim.api.nvim_set_option_value('bufhidden', 'wipe', { buf = new_buf })
                for _, msg in ipairs(out.message) do
                    local lines = vim.split(msg.code, '\\n')
                    vim.api.nvim_buf_set_lines(new_buf, 0, -1, false, lines)
                end
                vim.api.nvim_set_option_value('modifiable', false, { buf = new_buf })

                -- open a floating window centered over the editor
                local editor_width = vim.o.columns
                local editor_height = vim.o.lines
                local win_width = math.ceil(editor_width * 0.8)
                local win_height = math.ceil(editor_height * 0.8)
                local row = math.ceil((editor_height - win_height) / 2)
                local col = math.ceil((editor_width - win_width) / 2)
                local new_win = vim.api.nvim_open_win(new_buf, true, {
                    relative = 'editor',
                    width = win_width,
                    height = win_height,
                    row = row,
                    col = col,
                    border = 'rounded'
                })
                vim.api.nvim_set_option_value('number', false, { win = new_win })
                vim.api.nvim_set_option_value('relativenumber', false, { win = new_win })
                vim.api.nvim_set_option_value('diff', true, { win = new_win })
                -- vim.api.nvim_set_option_value('diff', true, { buf = new_buf })

                -- setup some autocmds and keybinds for the buffer
                vim.api.nvim_create_autocmd({ 'BufLeave' }, {
                    buffer = new_buf,
                    callback = function()
                        vim.api.nvim_win_close(new_win, true)
                        -- delete the autocmd
                        return true
                    end
                })
                vim.keymap.set('n', 'q', function()
                    vim.api.nvim_win_close(new_win, true)
                end, { buffer = new_buf })
            end

            -- Create a new window and display the command output
            -- local new_buf = vim.api.nvim_create_buf(false, true)
            -- vim.api.nvim_buf_set_lines(new_buf, 0, -1, false, vim.split(cmd_result.stdout, "\n"))
            -- local new_win = vim.api.nvim_open_win(new_buf, true, {
            --     relative = 'editor',
            --     width = 80,
            --     height = 20,
            --     row = 5,
            --     col = 5,
            -- })
        end,
        {
            desc = 'Chat with Q',
            nargs = '*'
        }
    )
end

M.setup()

return M
