# sentra skill
sentra skill add
1. 支持 git，http，file，zip等，尽可能包含skill可能的引入形式
2. 对于远程文件先下载，对比zip先解压到临时目录，对于目录直接引用即可
3. 然后走 sentra scan skill path 一样的逻辑，因此相关的参数都要有
4. 根据 --agent 参数，决定是否将技能添加到 agent 的技能列表中


sentra skill list