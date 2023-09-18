package com.tomcat.service;

import cn.hutool.core.lang.Tuple;
import com.tomcat.controller.requeset.ChatReq;
import com.tomcat.controller.response.*;
import com.tomcat.utils.ThreeTuple;
import com.tomcat.utils.TwoTuple;
import com.unfbx.chatgpt.entity.chat.ChatCompletion;
import com.unfbx.chatgpt.entity.chat.ChatCompletionResponse;
import com.unfbx.chatgpt.entity.chat.Message;
import com.unfbx.chatgpt.entity.common.Usage;

import java.util.List;

public interface AiCTutor {


    /**
     * @description 最新版的GPT-3.5 chat completion 更加贴近官方网站的问答模型
     * @param messages: 问答参数
     * @return ChatCompletionResponse: 答案
     * @author sm0007
     * @date 2023/8/29 2:21 PM
     */
    ChatCompletionResponse chatCompletion(List<Message> messages);

    ChatCompletionResponse chatCompletion(ChatCompletion chatCompletion);

    /**
     * @description 获取用户的聊天上下文
     * @param uid:
     * @return List<Message>
     * @author sm0007
     * @date 2023/8/29 7:14 PM
     */
    List<Message> getChatContext(String uid);

    /**
     * @description 初始化一个chat会话
     * @return uid
     * @author sm0007
     * @date 2023/8/29 8:18 PM
     */
    MsgIds chatInit(String uid);
    MsgIds chatInit(ChatReq req);

    /**
     * @description 获取课程表
     * @param :
     * @return ChatResp<List<Lesson>>
     * @author sm0007
     * @date 2023/8/30 4:15 PM
     */
    TwoTuple<TopicsResp, Usage> curriculumPlan(ChatReq req);


    /**
     * @description 开启指定topic对话
     * @param req:
     * @return ChatResp<ConversationData>
     * @author tomcat
     * @date 2023/9/4 3:05 PM
     */
    ThreeTuple<ChatAssistantDataResp, MsgIds, Usage> startTopic(ChatReq req);

    /**
     * @description 自定义topic对话
     * @param req:
     * @return ChatResp<ChatAssistantDataResp>
     * @author tomcat
     * @date 2023/9/15 4:00 PM
     */
    ThreeTuple<ChatAssistantDataResp, MsgIds, Usage> customizeTopic(ChatReq req);


    /**
     * @description 聊天
     * @param req:
     * @return ChatResp<ConversationData>
     * @author tomcat
     * @date 2023/9/4 3:36 PM
     */
    ThreeTuple<ChatAssistantDataResp, MsgIds, Usage> chat(ChatReq req);

    /**
     * @description 根据topic和sentence生成tips
     * @param req:
     * @return ChatResp<TipsResp>
     * @author tomcat
     * @date 2023/9/7 4:09 PM
     */
    TwoTuple<TipsResp, Usage> generateTips(ChatReq req);



    /**
     * @description 根据chatId获取离线消息
     * @param req:
     * @return ChatResp<TipsResp>
     * @author tomcat
     * @date 2023/9/11 3:11 PM
     */
    List<OfflineMsgResp> offlineMsg(ChatReq req);

    /**
     * @description 客户端在收到消息后的回执
     * @param req:
     * @return ChatResp
     * @author tomcat
     * @date 2023/9/12 4:04 PM
     */
    boolean msgConfirm(ChatReq req);



}
