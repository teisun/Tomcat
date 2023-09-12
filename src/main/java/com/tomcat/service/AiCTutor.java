package com.tomcat.service;

import com.tomcat.controller.requeset.ChatReq;
import com.tomcat.controller.response.*;
import com.unfbx.chatgpt.entity.chat.ChatCompletion;
import com.unfbx.chatgpt.entity.chat.ChatCompletionResponse;
import com.unfbx.chatgpt.entity.chat.Message;

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
    ChatResp<String> chatInit(String uid);

    /**
     * @description 获取课程表
     * @param :
     * @return ChatResp<List<Lesson>>
     * @author sm0007
     * @date 2023/8/30 4:15 PM
     */
    ChatResp<TopicsResp> curriculumPlan(ChatReq req);


    /**
     * @description 开启指定topic对话
     * @param req:
     * @return ChatResp<ConversationData>
     * @author tomcat
     * @date 2023/9/4 3:05 PM
     */
    ChatResp<ChatAssistantDataResp> startTopic(ChatReq req);


    /**
     * @description 聊天
     * @param req:
     * @return ChatResp<ConversationData>
     * @author tomcat
     * @date 2023/9/4 3:36 PM
     */
    ChatResp<ChatAssistantDataResp> chat(ChatReq req);

    /**
     * @description 根据topic和sentence生成tips
     * @param req:
     * @return ChatResp<TipsResp>
     * @author tomcat
     * @date 2023/9/7 4:09 PM
     */
    ChatResp<TipsResp> generateTips(ChatReq req);


    /**
     * @description 根据chatId获取离线消息
     * @param req:
     * @return ChatResp<TipsResp>
     * @author tomcat
     * @date 2023/9/11 3:11 PM
     */
    ChatResp<List<OfflineMsgResp>> offlineMsg(ChatReq req);

    /**
     * @description 客户端在收到消息后的回执
     * @param req:
     * @return ChatResp
     * @author tomcat
     * @date 2023/9/12 4:04 PM
     */
    ChatResp msgConfirm(ChatReq req);



}
