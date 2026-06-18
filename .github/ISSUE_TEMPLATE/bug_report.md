name: Bug Report
description: 报告一个 Bug
title: "[Bug]: "
labels: ["bug"]
body:
  - type: markdown
    attributes:
      value: |
        感谢你花时间报告这个 Bug！

  - type: input
    id: version
    attributes:
      label: 版本
      description: 你使用的 search_tool 版本
      placeholder: "0.1.0"
    validations:
      required: true

  - type: textarea
    id: description
    attributes:
      label: Bug 描述
      description: 请清晰简洁地描述这个 Bug
    validations:
      required: true

  - type: textarea
    id: reproduction
    attributes:
      label: 复现步骤
      description: 如何复现该行为
      placeholder: |
        1. 执行 '...'
        2. 输入 '....'
        3. 看到错误 '....'
    validations:
      required: true

  - type: textarea
    id: expected
    attributes:
      label: 预期行为
      description: 你期望发生什么
    validations:
      required: true

  - type: textarea
    id: context
    attributes:
      label: 额外信息
      description: 截图、日志、配置等
