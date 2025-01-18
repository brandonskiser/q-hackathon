local M = {}

---@param file_list string[]
---@param picker_buf integer
---@param window integer
local function create_file_picker(file_list, picker_buf, window)
    local pickers = require("telescope.pickers")
    local finders = require("telescope.finders")
    local conf = require("telescope.config").values
    local actions = require("telescope.actions")
    local action_state = require("telescope.actions.state")

    local files = vim.fn.globpath(vim.fn.getcwd(), "**/*", false, 1)
    pickers.new({
        buffer = picker_buf,
    }, {
        prompt_title = "Select Files to include as context. Use tab to select and enter to close",
        sorter = conf.file_sorter({}),
        finder = finders.new_table({
          results = files,
        }),
        attach_mappings = function(prompt_bufnr, _)
            actions.select_default:replace(function()
                local picker = action_state.get_current_picker(prompt_bufnr)
                local selections = picker:get_multi_selection()
                for _, selection in ipairs(selections) do
                    table.insert(file_list, selection.value)
                    print("Added: " .. selection.value)
                end
                actions.close(prompt_bufnr)
                if vim.api.nvim_win_is_valid(window) then
                    vim.api.nvim_set_current_win(window)
                end
            end)
            return true
        end,
    }):find()
end

M.create_file_picker = create_file_picker

return M
