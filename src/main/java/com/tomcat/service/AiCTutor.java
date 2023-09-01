package com.tomcat.service;

import com.tomcat.controller.requeset.ChatReq;
import com.tomcat.controller.response.ChatResp;
import com.tomcat.controller.response.Lesson;
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
    ChatResp<List<Lesson>> curriculumPlan(ChatReq req);

}
