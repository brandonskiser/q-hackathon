local M = {}

local state = {
    -- State for code requests.
    code = {
        open = false,
        file_win = -1,
        ai_win = -1,
    },
    -- State for chat conversations.
    chat = {
        open = false,
        chat_win = -1,
        prompt_win = -1,
    },
}

local CODE_AUGROUP_NAME = 'q-ai-code'
local CHAT_AUGROUP_NAME = 'q-ai-chat'

local MOCK = true
local TEST_FAILURE = false

local cmd_name = "hackathon"
local cmd_path = "/Volumes/workplace/hackathon/target/debug/" .. cmd_name

local function debug(...)
    print(...)
end

local MOCK_RES = [[
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
local function mock_res()
    local lines = vim.api.nvim_buf_get_lines(0, 0, -1, false)
    lines[#lines + 1] = "hello"
    return vim.json.encode({
        type = 'code',
        message = {
            {
                language = 'lua',
                code = vim.fn.join(lines, "\\n")
            }
        }
    })
end

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

local function close_chat()
    pcall(vim.api.nvim_win_close, state.chat.chat_win, true)
    pcall(vim.api.nvim_win_close, state.chat.prompt_win, true)
    pcall(vim.api.nvim_del_augroup_by_name, CHAT_AUGROUP_NAME)
    state.chat.open = false
end

local function close_code()
    pcall(vim.api.nvim_win_close, state.code.file_win, true)
    pcall(vim.api.nvim_win_close, state.code.ai_win, true)
    pcall(vim.api.nvim_del_augroup_by_name, CODE_AUGROUP_NAME)
    state.code.open = false
end

---@class CallOpts
---@field prompt string
---@field stdin? string | string[]

---Calls the binary for a chat session.
---@param opts CallOpts
---@return vim.SystemCompleted
local function call_chat(opts)
    local cmd_result = vim.system({ cmd_path, 'chat', opts.prompt },
        {
            text = true,
            stdin = opts.stdin,
            env = { RUST_LOG = 'debug' }
        }):wait()
    return cmd_result
end

local function make_chat_request(prompt)
end

---Calls the binary for a code modification request.
---@param opts CallOpts
---@return vim.SystemCompleted
local function call_code(opts)
    if MOCK then
        return {
            code = 0,
            signal = 0,
            stdout = mock_res()
        }
    end
    local cmd_result = vim.system({ cmd_path, '--code', opts.prompt },
        {
            text = true,
            stdin = opts.stdin,
            env = { RUST_LOG = 'debug' }
        }):wait()
    return cmd_result
end

---@class CodeObject
---@field language string
---@field code string
---@field file_path? string

---@class CodeResponse
---@field type 'code'
---@field message CodeObject[]

---Makes a request using the provided prompt.
---@param prompt string
---@return CodeResponse? code_response `CodeResponse` if successful, otherwise `nil`
local function code_request(prompt)
    local curr_buf = vim.api.nvim_buf_get_lines(0, 0, -1, false)
    local cmd_result = call_code({ prompt = prompt, stdin = curr_buf })
    if cmd_result.code ~= 0 then
        error('command failed with code: ' .. cmd_result.code .. '. stderr: ' .. cmd_result.stderr)
        return
    end
    return vim.json.decode(cmd_result.stdout)
end

---Sends the prompt currently stored in the chat prompt window, if not empty.
local function send_prompt()
end


---Initialize q-ai
---@param opts? table
function M.setup(opts)
    opts = opts or {}

    vim.api.nvim_create_user_command(
        'QChat',
        function(tab)
            if state.chat.open then return end

            local chat_buf = vim.api.nvim_create_buf(false, true)
            vim.bo[chat_buf].modifiable = false
            vim.bo[chat_buf].filetype = 'markdown'
            vim.bo[chat_buf].bufhidden = 'wipe'

            local prompt_buf = vim.api.nvim_create_buf(false, true)
            vim.bo[chat_buf].bufhidden = 'wipe'

            -- Create the chat and prompt windows.

            local editor_width = vim.o.columns
            local editor_height = vim.o.lines
            local win_width = math.ceil(editor_width * 0.8)
            local win_height = math.ceil(editor_height * 0.8)
            local row = math.ceil((editor_height - win_height) / 2)
            local col = math.ceil(editor_width * 0.1)
            local chat_win = vim.api.nvim_open_win(chat_buf, true, {
                relative = 'editor',
                width = win_width,
                height = win_height,
                row = row,
                col = col,
                zindex = 3,
                border = 'rounded',
            })
            state.chat.chat_win = chat_win

            local prompt_win = vim.api.nvim_open_win(prompt_buf, true, {
                relative = 'editor',
                width = win_width,
                height = 3,
                row = row + win_height,
                col = col,
                zindex = 3,
                border = 'rounded',
            })
            vim.wo[prompt_win].number = false
            vim.wo[prompt_win].relativenumber = false
            state.chat.prompt_win = prompt_win

            local chat_augroup = vim.api.nvim_create_augroup(CHAT_AUGROUP_NAME, { clear = true })
            vim.api.nvim_create_autocmd('BufEnter', {
                group = chat_augroup,
                callback = function(args)
                    if args.buf ~= chat_buf and args.buf ~= prompt_buf then
                        close_chat()
                    end
                end
            })

            vim.keymap.set('n', '<esc>', function() close_chat() end, { desc = 'close chat', buffer = chat_buf })
            vim.keymap.set('n', '<C-j>', function() vim.api.nvim_set_current_win(prompt_win) end,
                { desc = 'move to prompt window', buffer = chat_buf })
            vim.keymap.set('n', 'i',
                function()
                    vim.api.nvim_set_current_win(prompt_win)
                    vim.cmd('startinsert')
                end,
                { desc = 'move to prompt window in insert mode', buffer = chat_buf })
            vim.keymap.set('n', '<esc>', function() close_chat() end, { desc = 'close chat', buffer = prompt_buf })
            vim.keymap.set('n', 'q', function() close_chat() end, { desc = 'close chat', buffer = prompt_buf })
            vim.keymap.set('n', '<C-k>', function() vim.api.nvim_set_current_win(state.chat.chat_win) end,
                { desc = 'move to chat window', buffer = prompt_buf })
            vim.keymap.set('i', '<CR>', function()
                local prompt = vim.api.nvim_buf_get_lines(prompt_buf, 0, 1, false)
                debug('Sending: ' .. prompt[1])
                vim.api.nvim_buf_set_lines(prompt_buf, 0, -1, false, {})
            end, { desc = 'send prompt', buffer = prompt_buf })

            local prompt = tab.args
            if prompt ~= '' then
                local ok, result = pcall(make_chat_request, prompt)
                if not ok or not result then end
            end
        end,
        {
            desc = 'Talk with the AI',
            nargs = '*'
        }
    )

    vim.api.nvim_create_user_command(
        'QCode',
        function(tab)
            if state.code.open then return end
            local prompt = tab.args
            if prompt == '' then
                debug("No prompt provided, returning")
                return
            end

            local ok, response = pcall(code_request, prompt)
            if not ok or not response then return end

            -- create a new buffer to display the results
            local ai_buf = vim.api.nvim_create_buf(false, true)
            -- vim.api.nvim_set_option_value('bufhidden', 'wipe', { buf = new_buf })
            -- vim.api.nvim_set_option_value('filetype', vim.bo[0].filetype, { buf = new_buf })
            vim.bo[ai_buf].bufhidden = 'wipe'
            vim.bo[ai_buf].filetype = vim.bo[0].filetype
            for _, msg in ipairs(response.message) do
                local lines = vim.split(msg.code, '\\n')
                vim.api.nvim_buf_set_lines(ai_buf, 0, -1, false, lines)
            end
            -- vim.api.nvim_set_option_value('modifiable', false, { buf = new_buf })
            vim.bo[ai_buf].modifiable = false

            -- open two floating windows
            local editor_width = vim.o.columns
            local editor_height = vim.o.lines
            local win_width = math.ceil(editor_width * 0.4)
            local win_height = math.ceil(editor_height * 0.8)

            -- first window to hold the current buffer contents.
            -- this is hidden, and only used for getting the diff option
            -- set correctly.
            local row = math.ceil((editor_height - win_height) / 2)
            local col = math.ceil(editor_width * 0.1)
            local file_win = vim.api.nvim_open_win(0, true, {
                relative = 'editor',
                width = 1,
                height = 1,
                row = 0,
                col = 0,
                hide = true,
            })
            vim.api.nvim_set_option_value('diff', true, { scope = 'local' })
            state.code.file_win = file_win

            -- second window to hold the AI output
            win_width = math.ceil(editor_width * 0.8)
            row = math.ceil((editor_height - win_height) / 2)
            col = math.ceil(editor_width * 0.1)
            local ai_win = vim.api.nvim_open_win(ai_buf, true, {
                relative = 'editor',
                width = win_width,
                height = win_height,
                row = row,
                col = col + 1,
                border = 'rounded',
                zindex = 2,
            })
            vim.api.nvim_set_option_value('diff', true, { win = ai_win })
            state.code.ai_win = ai_win

            state.code.open = true

            local augroup = vim.api.nvim_create_augroup(CODE_AUGROUP_NAME, { clear = true })
            vim.api.nvim_create_autocmd('BufEnter', {
                group = augroup,
                callback = function(args)
                    -- debug(string.format('BufLeave: %s', vim.inspect(args)))
                    if args.buf ~= ai_buf then
                        close_code()
                    end
                end
            })
            vim.keymap.set('n', 'q', function() close_code() end, { buffer = ai_buf })
            vim.keymap.set('n', '<esc>', function() close_code() end, { buffer = ai_buf })
            vim.keymap.set('n', 'p', function() vim.cmd('diffput') end, { buffer = ai_buf })
        end,
        {
            desc = 'Make a code modification',
            nargs = '*'
        }
    )
end

M.setup()

return M
