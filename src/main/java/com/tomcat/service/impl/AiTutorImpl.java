package com.tomcat.service.impl;

import cn.hutool.core.util.StrUtil;
import cn.hutool.json.JSONUtil;
import com.tomcat.config.LocalCache;
import com.tomcat.controller.requeset.ChatReq;
import com.tomcat.controller.requeset.ChatUserData;
import com.tomcat.controller.requeset.TipsReq;
import com.tomcat.controller.response.*;
import com.tomcat.service.AiCTutor;
import com.tomcat.utils.UniqueIdentifierGenerator;
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
    @Value("${ai.prompt.curriculum.limiter}")
    private String promptCurriculumLimiter;
    @Value("${ai.prompt.chat.limiter}")
    private String promptChatLimiter;

    @Value("${ai.prompt.chat.sentence_checker}")
    private String promptSentenceChecker;

    @Value("${ai.prompt.chat.generateTips}")
    private String promptChatGenerateTips;

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

        // 保存chatId
        List<String> ids = (List<String>) LocalCache.CACHE.get(uid);
        if(ids == null){
            ids = new ArrayList<>();
        }
        String chatId = UniqueIdentifierGenerator.uniqueId();
        ids.add(chatId);
        LocalCache.CACHE.put(uid, ids);

        Message firstMsg = Message.builder().content(promptAitutor).role(Message.Role.USER).build();
        List<Message> messages = new ArrayList<>();
        messages.add(firstMsg);
        Message secondMsg = Message.builder().content(promptVersion).role(Message.Role.ASSISTANT).build();
        messages.add(secondMsg);
        LocalCache.CACHE.put(chatId, JSONUtil.toJsonStr(messages), LocalCache.TIMEOUT);
        log.info("chatInit: " + secondMsg.getContent());
        ChatResp<String> resp = new ChatResp<>();
        resp.setCode(200);
        resp.setCommand(Command.CHAT_INIT);
        resp.setData(chatId);
        return resp;
    }

    @Override
    public ChatResp<Topics> curriculumPlan(ChatReq req) {

        // 获取chat上下文
        String chatId = req.getChatId();
        String messageContext = (String) LocalCache.CACHE.get(chatId);
        log.info(Command.CURRICULUM_PLAN + " messageContext: " + messageContext);
        ChatResp<Topics> resp = new ChatResp<>();
        if (StrUtil.isNotBlank(messageContext)) {
            List<Message> messages = new ArrayList<>();
            messages = JSONUtil.toList(messageContext, Message.class);
            String content;
            String prompt_limit = " " + promptCurriculumLimiter; // 限时模型返回topic obj的数量
            if(StrUtil.isNotBlank(req.getData())){
                content = Command.CURRICULUM_PLAN + " "+ req.getData() + prompt_limit;
            }else {
                content = Command.CURRICULUM_PLAN + prompt_limit;
            }
            log.info(Command.CURRICULUM_PLAN + " currentMessage content: " + content);
            Message currentMessage = Message.builder().content(content).role(Message.Role.USER).build();
            messages.add(currentMessage);
            ChatCompletionResponse response = this.chatCompletion(messages);
            Message responseMag = response.getChoices().get(0).getMessage();
            log.info(Command.CURRICULUM_PLAN + " responseMag content: " + responseMag.getContent());

            Topics topics = JSONUtil.toBean(responseMag.getContent(), Topics.class);
            resp.setCode(200);
            resp.setCommand(Command.CURRICULUM_PLAN);
            resp.setData(topics);
            resp.setUsage(response.getUsage());

            // 将响应数据加入到上下文缓存中
            messages.add(responseMag);
            LocalCache.CACHE.put(chatId, JSONUtil.toJsonStr(messages), LocalCache.TIMEOUT);

        }else {
            resp.setCode(404);
            resp.setDescribe("chat context not found!");
        }
        return resp;
    }

    @Override
    public ChatResp<ChatAssistantData> startTopic(ChatReq req) {
        // 获取chat上下文
        String chatId = req.getChatId();
        String messageContext = (String) LocalCache.CACHE.get(chatId);
        log.info(Command.START_TOPIC + " messageContext: " + messageContext);
        ChatResp<ChatAssistantData> resp = new ChatResp<>();
        if (StrUtil.isNotBlank(messageContext)) {
            // 上下文list
            List<Message> messages = JSONUtil.toList(messageContext, Message.class);

            // 编辑prompt加入到上下文中
            String content;
            if(StrUtil.isNotBlank(req.getData())){
                content = Command.START_TOPIC + " "+ req.getData();
            }else {
                content = Command.START_TOPIC + " random";
            }
            log.info(Command.START_TOPIC + " currentMessage content: " + content);
            Message currentMessage = Message.builder().content(content).role(Message.Role.USER).build();
            messages.add(currentMessage);
            // 发送上下文到AI 获取返回的响应数据
            ChatCompletionResponse response = this.chatCompletion(messages);
            Message responseMag = response.getChoices().get(0).getMessage();
            log.info(Command.START_TOPIC + "  responseMag content: " + responseMag.getContent());


            // 将响应数据格式化成java bean返回请求端
            ChatAssistantData chatAssistantData = JSONUtil.toBean(responseMag.getContent(), ChatAssistantData.class);

            // 生成tips
            TipsReq tipsReq = new TipsReq();
            tipsReq.setTopic(chatAssistantData.getTopic());
            tipsReq.setQuestion(chatAssistantData.getAssistant_sentence());
            TipsResp tipsResp = generateTips(JSONUtil.toJsonStr(tipsReq));
            chatAssistantData.setTips(tipsResp.getTips());

            resp.setCode(200);
            resp.setCommand(Command.START_TOPIC);
            resp.setData(chatAssistantData);
            resp.setUsage(response.getUsage());
            resp.addUsage(tipsResp.getUsage());

            // 将响应数据加入到上下文缓存中
            messages.add(responseMag);
            LocalCache.CACHE.put(chatId, JSONUtil.toJsonStr(messages), LocalCache.TIMEOUT);
        }else {
            resp.setCode(404);
            resp.setDescribe("chat context not found!");
        }
        return resp;
    }

    @Override
    public ChatResp<ChatAssistantData> chat(ChatReq req) {

        log.info(Command.CHAT +" req data: " + req.getData());
        String chatId = req.getChatId();

        // 获取chat上下文
        String messageContext = (String) LocalCache.CACHE.get(chatId);
        log.info(Command.CHAT +" messageContext: " + messageContext);
        ChatResp<ChatAssistantData> resp = new ChatResp<>();
        if(StrUtil.isBlank(req.getData())){
            resp.setCode(404);
            resp.setDescribe("chat data must be not null!");
            return resp;
        }
        if (StrUtil.isNotBlank(messageContext)) {
            // 上下文list
            List<Message> messages = JSONUtil.toList(messageContext, Message.class);
            // 检查user_sentence语法
            Message messageCheckSentence = Message.builder().content("sentence:"+ req.getData()+ " \n"+promptSentenceChecker).role(Message.Role.USER).build();
            List<Message> messagesCheckSentence = new ArrayList<>();
            messagesCheckSentence.add(messageCheckSentence);
            ChatCompletionResponse checkSentenceResponse = this.chatCompletion(messagesCheckSentence);
            String checkSentenceResponseStr = checkSentenceResponse.getChoices().get(0).getMessage().getContent();
            ChatAssistantData.Suggestion suggestion = JSONUtil.toBean(checkSentenceResponseStr, ChatAssistantData.Suggestion.class);
            log.info(Command.CHAT + " checkSentenceResponse content: " + checkSentenceResponseStr);
            log.info(Command.CHAT + " checkSentenceResponse usage: " + checkSentenceResponse.getUsage());

            // 编辑prompt加入到上下文中
            ChatUserData chatUserData = new ChatUserData();
            chatUserData.setUser_sentence(req.getData());
            chatUserData.setPrompt(promptChatLimiter);
            String content = JSONUtil.toJsonStr(chatUserData);
            log.info(Command.CHAT + " currentMessage content: " + content);
            Message currentMessage = Message.builder().content(content).role(Message.Role.USER).build();
            messages.add(currentMessage);
            // 发送上下文到AI 获取返回的响应数据
            ChatCompletionResponse response = this.chatCompletion(messages);
            Message responseMag = response.getChoices().get(0).getMessage();
            log.info(Command.CHAT + " responseMag content: " + responseMag.getContent());

            // 将响应数据格式化成java bean返回请求端
            ChatAssistantData chatAssistantData = JSONUtil.toBean(responseMag.getContent(), ChatAssistantData.class);

            // 生成tips
            TipsReq tipsReq = new TipsReq();
            tipsReq.setTopic(chatAssistantData.getTopic());
            tipsReq.setQuestion(chatAssistantData.getAssistant_sentence());
            TipsResp tipsResp = generateTips(JSONUtil.toJsonStr(tipsReq));

            chatAssistantData.setTips(tipsResp.getTips());
            chatAssistantData.setSuggestion(suggestion);
            resp.setCode(200);
            resp.setCommand(Command.CHAT);
            resp.setData(chatAssistantData);
            resp.setUsage(response.getUsage());
            resp.addUsage(tipsResp.getUsage());

            // 将响应数据加入到上下文缓存中
            messages.add(responseMag);
            LocalCache.CACHE.put(chatId, JSONUtil.toJsonStr(messages), LocalCache.TIMEOUT);
        }else {
            resp.setCode(404);
            resp.setDescribe("chat context not found!");
        }
        return resp;
    }

    @Override
    public ChatResp<TipsResp> generateTips(ChatReq req) {
        ChatResp<TipsResp> resp = new ChatResp<>();
        if(StrUtil.isBlank(req.getData())){
            resp.setCode(404);
            resp.setDescribe("Req data must be not null!");
            return resp;
        }
        TipsResp tipsResp = generateTips(req.getData());
        resp.setCode(200);
        resp.setCommand(Command.TIPS);
        resp.setData(tipsResp);
        resp.setUsage(tipsResp.getUsage());

        return resp;
    }

    @Override
    public ChatResp<List<OfflineMsgDTO>> offlineMsg(ChatReq req) {
        ChatResp<List<OfflineMsgDTO>> resp = new ChatResp<>();
        if(!LocalCache.MESSAGE_CACHE.containsKey(req.getUid())){
            resp.setCode(404);
            resp.setDescribe("Req data must be not null!");
        }
        List<OfflineMsgDTO> list = (List<OfflineMsgDTO>) LocalCache.MESSAGE_CACHE.get(req.getUid());
        resp.setData(list);
        return resp;
    }

    private TipsResp generateTips(String tipsReq){
        Message messageGenerateTips = Message.builder().content(tipsReq+ "\n"+promptChatGenerateTips).role(Message.Role.USER).build();
        List<Message> messages = new ArrayList<>();
        messages.add(messageGenerateTips);
        ChatCompletionResponse response = this.chatCompletion(messages);
        String responseStr = response.getChoices().get(0).getMessage().getContent();
        TipsResp tipsResp = JSONUtil.toBean(responseStr, TipsResp.class);
        tipsResp.setUsage(response.getUsage());
        log.info(Command.TIPS + " " + tipsReq);
        log.info(Command.TIPS + " generateTips content: " + responseStr);
        log.info(Command.TIPS + " generateTips usage: " + response.getUsage());
        return tipsResp;
    }


}
