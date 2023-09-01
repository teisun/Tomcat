package com.tomcat.service.impl;

import cn.hutool.core.util.StrUtil;
import cn.hutool.json.JSONUtil;
import com.tomcat.config.LocalCache;
import com.tomcat.controller.requeset.ChatReq;
import com.tomcat.controller.response.ChatResp;
import com.tomcat.controller.response.Lesson;
import com.tomcat.service.AiCTutor;
import com.tomcat.websocket.Command;
import com.unfbx.chatgpt.OpenAiClient;
import com.unfbx.chatgpt.entity.chat.ChatCompletion;
import com.unfbx.chatgpt.entity.chat.ChatCompletionResponse;
import com.unfbx.chatgpt.entity.chat.Message;
import lombok.extern.slf4j.Slf4j;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Service;

import java.util.ArrayList;
import java.util.List;

/**
 * @projectName: Tomcat
 * @package: com.tomcat.service
 * @className: AiCLient
 * @author: tomcat
 * @description: TODO
 * @date: 2023/8/29 2:35 PM
 * @version: 1.0
 */

@Service
@Slf4j
public class AiTutorImpl implements AiCTutor {


    @Autowired
    private OpenAiClient aiClient;

    @Value("${ai.prompt.aitutor}")
    private String promptAitutor;
    @Value("${ai.prompt.version}")
    private String promptVersion;
    @Value("${ai.prompt.curriculum.limit}")
    private String promptCurriculumLimit;

    @Override
    public ChatCompletionResponse chatCompletion(List<Message> messages) {
        ChatCompletion completion = ChatCompletion
                .builder()
                .messages(messages)
                .model(ChatCompletion.Model.GPT_3_5_TURBO_16K.getName())
                .build();
        return aiClient.chatCompletion(completion);
    }

    @Override
    public ChatCompletionResponse chatCompletion(ChatCompletion chatCompletion) {
        chatCompletion.setModel(ChatCompletion.Model.GPT_3_5_TURBO_16K.getName());
        return aiClient.chatCompletion(chatCompletion);
    }

    @Override
    public List<Message> getChatContext(String uid) {
        return null;
    }

    @Override
    public ChatResp<String> chatInit(String uid) {
        Message firstMsg = Message.builder().content(promptAitutor).role(Message.Role.USER).build();
        List<Message> messages = new ArrayList<>();
        messages.add(firstMsg);
//        ChatCompletionResponse response = this.chatCompletion(messages);
//        Message responseMag = response.getChoices().get(0).getMessage();
        Message secondMsg = Message.builder().content(promptVersion).role(Message.Role.ASSISTANT).build();
        messages.add(secondMsg);
        LocalCache.CACHE.put(uid, JSONUtil.toJsonStr(messages), LocalCache.TIMEOUT);
        log.info("AiCTutorImpl chatInit: " + secondMsg.getContent());
        ChatResp<String> resp = new ChatResp<>();
        resp.setCode(200);
        resp.setData(uid);
        return resp;
    }

    @Override
    public ChatResp<List<Lesson>> curriculumPlan(ChatReq req) {
        // 获取chat上下文
        String messageContext = (String) LocalCache.CACHE.get(req.getUid());
        log.info("AiCTutorImpl messageContext: " + messageContext);
        ChatResp<List<Lesson>> resp = new ChatResp<>();
        if (StrUtil.isNotBlank(messageContext)) {
            List<Message> messages = new ArrayList<>();
            messages = JSONUtil.toList(messageContext, Message.class);
            String content;
            String prompt_limit = promptCurriculumLimit; // 限时模型返回topic obj的数量
            if(StrUtil.isNotBlank(req.getData())){
                content = Command.CURRICULUM_PLAN + " "+ req.getData() + prompt_limit;
            }else {
                content = Command.CURRICULUM_PLAN + prompt_limit;
            }
            Message currentMessage = Message.builder().content(content).role(Message.Role.USER).build();
            messages.add(currentMessage);
            ChatCompletionResponse response = this.chatCompletion(messages);
            Message responseMag = response.getChoices().get(0).getMessage();
            log.info("AiCTutorImpl curriculumPlan content: " + responseMag.getContent());
            messages.add(responseMag);
            LocalCache.CACHE.put(req.getUid(), JSONUtil.toJsonStr(messages), LocalCache.TIMEOUT);

            List<Lesson> lessons = JSONUtil.toList(responseMag.getContent(), Lesson.class);
            resp.setCode(200);
            resp.setData(lessons);
            resp.setUsage(response.getUsage());
        }else {
            resp.setCode(404);
            resp.setDescribe("chat context not found!");
        }
        return resp;
    }


}
