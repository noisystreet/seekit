name: Feature Request
description: 建议一个新功能
title: "[Feature]: "
labels: ["enhancement"]
body:
  - type: markdown
    attributes:
      value: |
        感谢你的建议！

  - type: textarea
    id: problem
    attributes:
      label: 解决的问题
      description: 这个功能解决什么问题？
    validations:
      required: true

  - type: textarea
    id: solution
    attributes:
      label: 解决方案
      description: 你期望的行为或设计思路
    validations:
      required: true

  - type: textarea
    id: alternatives
    attributes:
      label: 替代方案
      description: 你考虑过的替代方案

  - type: textarea
    id: context
    attributes:
      label: 额外信息
      description: 参考实现、截图等
